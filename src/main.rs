use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use gix::hash::ObjectId;
use gix::refs::transaction::PreviousValue;

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
            target_branch,
            tips,
        } => run_branch(force, base.as_deref(), &target_branch, &tips),
    }
}

fn run_branch(force: bool, base: Option<&str>, target_branch: &str, tips: &[String]) -> Result<()> {
    if tips.is_empty() {
        bail!("at least one tip is required");
    }

    let cwd = std::env::current_dir().context("failed to read current directory")?;
    let repo = gix::discover(cwd).context("failed to discover git repository")?;
    let target_ref = format!("refs/heads/{target_branch}");

    let target_exists = repo
        .try_find_reference(target_ref.as_str())
        .context("failed to inspect target branch")?
        .is_some();
    if target_exists && !force {
        bail!("target branch '{target_branch}' already exists; re-run with --force to replace it");
    }

    let base_ref = match base {
        Some(base) => {
            let base_ref = resolve_base_ref(&repo, base)?;
            println!("selected base: {}", base_ref.name);
            base_ref
        }
        None => {
            let default_branch = detect_default_branch(&repo)?;
            println!("detected default branch: {}", default_branch.name);
            BaseRef {
                name: default_branch.name,
                commit_id: default_branch.commit_id,
            }
        }
    };
    println!("target branch: {target_branch}");

    let resolved_tips = resolve_tips(&repo, tips)?;
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

    Ok(())
}

fn detect_default_branch(repo: &gix::Repository) -> Result<DefaultBranch> {
    if let Some(mut remote_head) = repo
        .try_find_reference("refs/remotes/origin/HEAD")
        .context("failed to inspect origin/HEAD")?
    {
        if let Some(followed) = remote_head.follow() {
            let mut followed = followed.context("failed to resolve origin/HEAD")?;
            let commit = followed
                .peel_to_commit()
                .context("failed to peel origin/HEAD to a commit")?;
            return Ok(DefaultBranch {
                name: full_name_to_string(followed.name()),
                commit_id: commit.id,
            });
        }
        let commit = remote_head
            .peel_to_commit()
            .context("failed to peel origin/HEAD to a commit")?;
        return Ok(DefaultBranch {
            name: full_name_to_string(remote_head.name()),
            commit_id: commit.id,
        });
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
