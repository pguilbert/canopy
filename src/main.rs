use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use gix::config::tree::User;
use gix::hash::ObjectId;
use gix::refs::transaction::PreviousValue;
use std::process::Command;

const DEFAULT_COMMITTER_NAME: &str = "canopy[bot]";
const DEFAULT_COMMITTER_EMAIL: &str = "canopy@pguilbert.dev";

#[derive(Parser, Debug)]
#[command(name = "canopy")]
#[command(about = "Create temporary integration branches")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Branch {
        #[arg(long)]
        force: bool,
        #[arg(long)]
        base: Option<String>,
        #[arg(long)]
        remote: Option<String>,
        #[arg(long)]
        push: bool,
        target_branch: String,
        tips: Vec<String>,
    },
}

#[derive(Clone, Debug)]
struct DefaultBranch {
    name: String,
    commit_id: ObjectId,
}

#[derive(Clone, Debug)]
struct BaseRef {
    name: String,
    commit_id: ObjectId,
}

#[derive(Clone, Debug)]
struct ResolvedTip {
    input: String,
    commit_id: ObjectId,
}

#[derive(Clone, Debug)]
struct RemoteBranchInput {
    source: String,
    local_ref: String,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Branch {
            force,
            base,
            remote,
            push,
            target_branch,
            tips,
        } => run_branch(
            force,
            base.as_deref(),
            remote.as_deref(),
            push,
            &target_branch,
            &tips,
        ),
    }
}

fn run_branch(
    force: bool,
    base: Option<&str>,
    remote: Option<&str>,
    push: bool,
    target_branch: &str,
    tips: &[String],
) -> Result<()> {
    if tips.is_empty() {
        bail!("at least one tip is required");
    }

    if push && remote.is_none() {
        bail!("--push requires --remote");
    }

    let cwd = std::env::current_dir().context("failed to read current directory")?;
    let mut repo = gix::discover(cwd).context("failed to discover git repository")?;
    ensure_commit_identity(&mut repo)?;

    let (base, tips) = match remote {
        Some(remote) => prepare_remote_refs(remote, base, tips)?,
        None => (base.map(str::to_owned), tips.to_vec()),
    };

    let target_ref = format!("refs/heads/{target_branch}");

    let target_exists = repo
        .try_find_reference(target_ref.as_str())
        .context("failed to inspect target branch")?
        .is_some();
    if target_exists && !force {
        bail!("target branch '{target_branch}' already exists; re-run with --force to replace it");
    }

    let base_ref = match base.as_deref() {
        Some(base) => {
            let base_ref = resolve_base_ref(&repo, base)?;
            println!("selected base: {}", base_ref.name);
            base_ref
        }
        None => {
            let default_branch = detect_default_branch(&repo, remote)?;
            println!("detected default branch: {}", default_branch.name);
            BaseRef {
                name: default_branch.name,
                commit_id: default_branch.commit_id,
            }
        }
    };
    println!("target branch: {target_branch}");

    let resolved_tips = resolve_tips(&repo, &tips)?;
    println!("resolved tips:");
    for tip in &resolved_tips {
        println!("  {} -> {}", tip.input, tip.commit_id);
    }

    let deduplicated_tips = deduplicate_tips(&repo, resolved_tips)?;
    println!("deduplicated tips:");
    for tip in &deduplicated_tips {
        println!("  {} -> {}", tip.input, tip.commit_id);
    }

    let mut current_commit = base_ref.commit_id;
    let mut successful_merges = Vec::new();
    let mut failed_merges = Vec::new();

    for tip in &deduplicated_tips {
        println!("attempting merge: {} ({})", tip.input, tip.commit_id);
        let merge_options: gix::merge::tree::Options = repo
            .tree_merge_options()
            .context("failed to prepare merge options")?
            .into();
        let merge_options =
            gix::merge::commit::Options::from(merge_options).with_allow_missing_merge_base(true);
        let mut outcome = repo
            .merge_commits(
                current_commit,
                tip.commit_id,
                Default::default(),
                merge_options,
            )
            .with_context(|| format!("failed to merge tip '{}'", tip.input))?;

        if outcome
            .tree_merge
            .has_unresolved_conflicts(gix::merge::tree::TreatAsUnresolved::git())
        {
            println!(
                "failed merge due to conflicts: {} ({})",
                tip.input, tip.commit_id
            );
            failed_merges.push(tip.clone());
            continue;
        }

        let tree_id = outcome
            .tree_merge
            .tree
            .write()
            .with_context(|| format!("failed to write merged tree for '{}'", tip.input))?;
        let merge_commit = repo
            .new_commit(
                format!("Merge {} into {}", tip.input, target_branch),
                tree_id,
                [current_commit, tip.commit_id],
            )
            .with_context(|| format!("failed to write merge commit for '{}'", tip.input))?;
        current_commit = merge_commit.id;
        println!("successful merge: {} -> {}", tip.input, current_commit);
        successful_merges.push(tip.clone());
    }

    if !successful_merges.is_empty() {
        println!("successful merges:");
        for tip in &successful_merges {
            println!("  {} -> {}", tip.input, tip.commit_id);
        }
    }

    if !failed_merges.is_empty() {
        println!("failed merges due to conflicts:");
        for tip in &failed_merges {
            println!("  {} -> {}", tip.input, tip.commit_id);
        }
    }

    let constraint = if force {
        PreviousValue::Any
    } else {
        PreviousValue::MustNotExist
    };
    repo.reference(
        target_ref.as_str(),
        current_commit,
        constraint,
        format!("canopy branch {target_branch}"),
    )
    .with_context(|| format!("failed to update target branch '{target_branch}'"))?;

    if target_exists {
        println!("final branch update: replaced {target_branch} -> {current_commit}");
    } else {
        println!("final branch creation: created {target_branch} -> {current_commit}");
    }

    if let Some(remote) = remote.filter(|_| push) {
        push_branch(remote, target_branch)?;
    }

    Ok(())
}

fn detect_default_branch(repo: &gix::Repository, remote: Option<&str>) -> Result<DefaultBranch> {
    let remote_name = remote.unwrap_or("origin");
    let remote_head_name = format!("refs/remotes/{remote_name}/HEAD");

    if let Some(mut remote_head) = repo
        .try_find_reference(remote_head_name.as_str())
        .with_context(|| format!("failed to inspect {remote_name}/HEAD"))?
    {
        if let Some(followed) = remote_head.follow() {
            let mut followed =
                followed.with_context(|| format!("failed to resolve {remote_name}/HEAD"))?;
            let commit = followed
                .peel_to_commit()
                .with_context(|| format!("failed to peel {remote_name}/HEAD to a commit"))?;
            return Ok(DefaultBranch {
                name: full_name_to_string(followed.name()),
                commit_id: commit.id,
            });
        }
        let commit = remote_head
            .peel_to_commit()
            .with_context(|| format!("failed to peel {remote_name}/HEAD to a commit"))?;
        return Ok(DefaultBranch {
            name: full_name_to_string(remote_head.name()),
            commit_id: commit.id,
        });
    }

    if remote.is_some() {
        bail!(
            "could not determine default branch for remote '{remote_name}'; pass --base explicitly"
        )
    }

    if let Some(head_ref) = repo.head_ref().context("failed to inspect HEAD")? {
        if let Some(tracking_name) =
            head_ref.remote_tracking_ref_name(gix::remote::Direction::Fetch)
        {
            let tracking_name = tracking_name.context("failed to resolve tracking branch")?;
            let mut tracking_ref = repo
                .find_reference(tracking_name.as_ref())
                .context("failed to open tracking branch")?;
            let commit = tracking_ref
                .peel_to_commit()
                .context("failed to peel tracking branch to a commit")?;
            return Ok(DefaultBranch {
                name: full_name_to_string(tracking_ref.name()),
                commit_id: commit.id,
            });
        }
    }

    if let Some(head_name) = repo.head_name().context("failed to inspect HEAD name")? {
        let mut head_ref = repo
            .find_reference(head_name.as_ref())
            .context("failed to open current branch")?;
        let commit = head_ref
            .peel_to_commit()
            .context("failed to peel current branch to a commit")?;
        return Ok(DefaultBranch {
            name: full_name_to_string(head_ref.name()),
            commit_id: commit.id,
        });
    }

    bail!("could not determine a default branch from local repository state")
}

fn resolve_base_ref(repo: &gix::Repository, base: &str) -> Result<BaseRef> {
    let object = repo
        .rev_parse_single(base)
        .with_context(|| format!("failed to resolve base '{base}'"))?;
    let commit = object
        .object()
        .with_context(|| format!("failed to load object for base '{base}'"))?
        .peel_to_commit()
        .with_context(|| format!("base '{base}' does not resolve to a commit"))?;

    Ok(BaseRef {
        name: base.to_owned(),
        commit_id: commit.id,
    })
}

fn resolve_tips(repo: &gix::Repository, tips: &[String]) -> Result<Vec<ResolvedTip>> {
    tips.iter()
        .map(|tip| {
            let object = repo
                .rev_parse_single(tip.as_str())
                .with_context(|| format!("failed to resolve tip '{tip}'"))?;
            let commit = object
                .object()
                .with_context(|| format!("failed to load object for tip '{tip}'"))?
                .peel_to_commit()
                .with_context(|| format!("tip '{tip}' does not resolve to a commit"))?;
            Ok(ResolvedTip {
                input: tip.clone(),
                commit_id: commit.id,
            })
        })
        .collect()
}

fn deduplicate_tips(repo: &gix::Repository, tips: Vec<ResolvedTip>) -> Result<Vec<ResolvedTip>> {
    let mut retained = Vec::new();

    for candidate in tips {
        retained.retain(|existing: &ResolvedTip| {
            !is_ancestor_or_same(repo, existing.commit_id, candidate.commit_id).unwrap_or(false)
        });

        let candidate_is_redundant = retained.iter().any(|existing| {
            is_ancestor_or_same(repo, candidate.commit_id, existing.commit_id).unwrap_or(false)
        });

        if !candidate_is_redundant {
            retained.push(candidate);
        }
    }

    Ok(retained)
}

fn ensure_commit_identity(repo: &mut gix::Repository) -> Result<()> {
    let config = repo.config_snapshot();
    let missing_user_name = config.string(User::NAME).is_none();
    let missing_user_email = config.string(User::EMAIL).is_none();

    if !missing_user_name && !missing_user_email {
        return Ok(());
    }

    let mut config = repo.config_snapshot_mut();
    if missing_user_name {
        config
            .set_value(&User::NAME, DEFAULT_COMMITTER_NAME)
            .context("failed to set fallback user.name")?;
    }
    if missing_user_email {
        config
            .set_value(&User::EMAIL, DEFAULT_COMMITTER_EMAIL)
            .context("failed to set fallback user.email")?;
    }
    config
        .commit()
        .context("failed to apply fallback commit identity")?;

    Ok(())
}

fn prepare_remote_refs(
    remote: &str,
    base: Option<&str>,
    tips: &[String],
) -> Result<(Option<String>, Vec<String>)> {
    let base_branch = base
        .map(|value| normalize_remote_branch_input(remote, value))
        .transpose()?;
    let tip_branches = tips
        .iter()
        .map(|tip| normalize_remote_branch_input(remote, tip))
        .collect::<Result<Vec<_>>>()?;

    fetch_remote_branches(remote, base_branch.as_ref(), &tip_branches)?;

    let base_ref = base_branch.map(|branch| branch.local_ref);
    let tip_refs = tip_branches
        .into_iter()
        .map(|branch| branch.local_ref)
        .collect();
    Ok((base_ref, tip_refs))
}

fn normalize_remote_branch_input(remote: &str, value: &str) -> Result<RemoteBranchInput> {
    let branch = if let Some(branch) = value.strip_prefix("refs/remotes/") {
        let (actual_remote, branch) = branch
            .split_once('/')
            .with_context(|| format!("remote ref '{value}' is missing a branch name"))?;
        if actual_remote != remote {
            bail!("remote ref '{value}' does not belong to remote '{remote}'")
        }
        branch.to_owned()
    } else if let Some(branch) = value.strip_prefix("refs/heads/") {
        branch.to_owned()
    } else if value.starts_with("refs/") {
        bail!("remote mode only supports branch names or refs/heads/* inputs, got '{value}'")
    } else {
        value.to_owned()
    };

    if branch.is_empty() {
        bail!("remote branch input must not be empty")
    }

    Ok(RemoteBranchInput {
        source: branch.clone(),
        local_ref: format!("refs/remotes/{remote}/{branch}"),
    })
}

fn fetch_remote_branches(
    remote: &str,
    base: Option<&RemoteBranchInput>,
    tips: &[RemoteBranchInput],
) -> Result<()> {
    let mut fetch_specs = Vec::new();
    if let Some(base) = base {
        fetch_specs.push(format!("+refs/heads/{}:{}", base.source, base.local_ref));
    }
    for tip in tips {
        fetch_specs.push(format!("+refs/heads/{}:{}", tip.source, tip.local_ref));
    }

    println!("fetching refs from remote: {remote}");
    run_git(&["fetch", "--no-tags", remote], fetch_specs)?;
    Ok(())
}

fn push_branch(remote: &str, target_branch: &str) -> Result<()> {
    let refspec = format!("+refs/heads/{target_branch}:refs/heads/{target_branch}");
    println!("pushing branch to remote: {remote}/{target_branch}");
    run_git(&["push", remote], vec![refspec])
}

fn run_git<'a>(prefix_args: &[&str], args: Vec<String>) -> Result<()> {
    let output = Command::new("git")
        .args(prefix_args)
        .args(&args)
        .output()
        .context("failed to run git")?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let printable_args = prefix_args
        .iter()
        .map(|arg| (*arg).to_owned())
        .chain(args)
        .collect::<Vec<_>>()
        .join(" ");

    bail!(
        "git {} failed\nstdout:\n{}\nstderr:\n{}",
        printable_args,
        stdout.trim_end(),
        stderr.trim_end()
    )
}

fn is_ancestor_or_same(repo: &gix::Repository, left: ObjectId, right: ObjectId) -> Result<bool> {
    if left == right {
        return Ok(true);
    }

    match repo.merge_base(left, right) {
        Ok(base) => Ok(base.detach() == left),
        Err(gix::repository::merge_base::Error::NotFound { .. }) => Ok(false),
        Err(err) => Err(err).context("failed to compare commit ancestry"),
    }
}

fn full_name_to_string(name: &gix::refs::FullNameRef) -> String {
    name.as_bstr().to_string()
}
