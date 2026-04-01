mod support;

use support::Fixture;

#[test]
fn creates_branch_and_deduplicates_ancestor_tips() {
    let repo = Fixture::sample_repo();
    let feature_one = repo
        .git_output(&["rev-parse", "linear-1"])
        .trim()
        .to_owned();
    let feature_two = repo
        .git_output(&["rev-parse", "linear-2"])
        .trim()
        .to_owned();

    let output = repo.canopy(&["branch", "integration", &feature_one, &feature_two, "other"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("detected default branch: refs/heads/main"));
    assert!(stdout.contains("deduplicated tips:"));
    let dedup_section = stdout
        .split("deduplicated tips:\n")
        .nth(1)
        .and_then(|section| section.split("attempting merge:").next())
        .expect("dedup section");
    assert!(!dedup_section.contains(&format!("  {} ->", feature_one)));
    assert!(dedup_section.contains(&format!("  {} ->", feature_two)));
    assert!(stdout.contains("final branch creation: created integration ->"));
    repo.assert_branch_exists("integration");
    repo.assert_branch_file("integration", "story.txt", "feature two\n");
    repo.assert_branch_file("integration", "other.txt", "other\n");
}

#[test]
fn accepts_mixed_commit_sha_and_branch_name_inputs() {
    let repo = Fixture::sample_repo();
    let linear_tip = repo
        .git_output(&["rev-parse", "linear-2"])
        .trim()
        .to_owned();

    let output = repo.canopy(&["branch", "integration", &linear_tip, "other"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&format!("  {} ->", linear_tip)));
    assert!(stdout.contains("  other ->"));
    repo.assert_branch_exists("integration");
    repo.assert_branch_file("integration", "story.txt", "feature two\n");
    repo.assert_branch_file("integration", "other.txt", "other\n");
}

#[test]
fn skips_conflicting_tip_and_continues() {
    let repo = Fixture::sample_repo();

    let output = repo.canopy(&[
        "branch",
        "integration",
        "feature-a",
        "feature-b",
        "feature-c",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("successful merge: feature-a"));
    assert!(stdout.contains("failed merge due to conflicts: feature-b"));
    assert!(stdout.contains("successful merge: feature-c"));
    repo.assert_branch_exists("integration");
    repo.assert_branch_file("integration", "story.txt", "feature a\n");
    repo.assert_branch_file("integration", "extra.txt", "feature c\n");
}

#[test]
fn refuses_existing_target_without_force() {
    let repo = Fixture::sample_repo();
    repo.git(&["branch", "integration"]);

    let output = repo.canopy(&["branch", "integration", "HEAD"]);
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already exists"));
}

#[test]
fn updates_existing_target_with_force() {
    let repo = Fixture::sample_repo();
    repo.git(&["branch", "integration"]);

    let output = repo.canopy(&["branch", "--force", "integration", "linear"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("final branch update: replaced integration ->"));
    repo.assert_branch_exists("integration");
    repo.assert_branch_file("integration", "story.txt", "feature two\n");
}
