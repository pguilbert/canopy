use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

#[test]
fn creates_branch_and_deduplicates_ancestor_tips() {
    let repo = TestRepo::new();

    repo.write_file("story.txt", "base\n");
    repo.git(&["add", "story.txt"]);
    repo.git(&["commit", "-m", "base"]);

    repo.git(&["checkout", "-b", "feature"]);
    repo.write_file("story.txt", "feature one\n");
    repo.git(&["commit", "-am", "feature one"]);
    let feature_one = repo.git_output(&["rev-parse", "HEAD"]);

    repo.write_file("story.txt", "feature two\n");
    repo.git(&["commit", "-am", "feature two"]);
    let feature_two = repo.git_output(&["rev-parse", "HEAD"]);

    repo.git(&["checkout", "main"]);
    repo.git(&["checkout", "-b", "other"]);
    repo.write_file("other.txt", "other\n");
    repo.git(&["add", "other.txt"]);
    repo.git(&["commit", "-m", "other"]);

    repo.git(&["checkout", "main"]);

    let output = repo.canopy(&[
        "branch",
        "integration",
        feature_one.trim(),
        feature_two.trim(),
        "other",
    ]);
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
    assert!(!dedup_section.contains(&format!("  {} ->", feature_one.trim())));
    assert!(dedup_section.contains(&format!("  {} ->", feature_two.trim())));
    assert!(stdout.contains("final branch creation: created integration ->"));
    assert_eq!(
        repo.git_output(&["show", "integration:other.txt"]),
        "other\n"
    );
}

#[test]
fn skips_conflicting_tip_and_continues() {
    let repo = TestRepo::new();

    repo.write_file("story.txt", "base\n");
    repo.git(&["add", "story.txt"]);
    repo.git(&["commit", "-m", "base"]);

    repo.git(&["checkout", "-b", "feature-a"]);
    repo.write_file("story.txt", "feature a\n");
    repo.git(&["commit", "-am", "feature a"]);

    repo.git(&["checkout", "main"]);
    repo.git(&["checkout", "-b", "feature-b"]);
    repo.write_file("story.txt", "feature b\n");
    repo.git(&["commit", "-am", "feature b"]);

    repo.git(&["checkout", "main"]);
    repo.git(&["checkout", "-b", "feature-c"]);
    repo.write_file("extra.txt", "feature c\n");
    repo.git(&["add", "extra.txt"]);
    repo.git(&["commit", "-m", "feature c"]);

    repo.git(&["checkout", "main"]);

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
    assert_eq!(
        repo.git_output(&["show", "integration:story.txt"]),
        "feature a\n"
    );
    assert_eq!(
        repo.git_output(&["show", "integration:extra.txt"]),
        "feature c\n"
    );
}

#[test]
fn refuses_existing_target_without_force() {
    let repo = TestRepo::new();

    repo.write_file("story.txt", "base\n");
    repo.git(&["add", "story.txt"]);
    repo.git(&["commit", "-m", "base"]);
    repo.git(&["branch", "integration"]);

    let output = repo.canopy(&["branch", "integration", "HEAD"]);
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already exists"));
}

#[test]
fn updates_existing_target_with_force() {
    let repo = TestRepo::new();

    repo.write_file("story.txt", "base\n");
    repo.git(&["add", "story.txt"]);
    repo.git(&["commit", "-m", "base"]);
    repo.git(&["branch", "integration"]);

    repo.git(&["checkout", "-b", "feature"]);
    repo.write_file("story.txt", "updated\n");
    repo.git(&["commit", "-am", "feature"]);
    repo.git(&["checkout", "main"]);

    let output = repo.canopy(&["branch", "--force", "integration", "feature"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("final branch update: replaced integration ->"));
    assert_eq!(
        repo.git_output(&["show", "integration:story.txt"]),
        "updated\n"
    );
}

struct TestRepo {
    dir: TempDir,
}

impl TestRepo {
    fn new() -> Self {
        let dir = TempDir::new().expect("tempdir");
        let repo = Self { dir };
        repo.git(&["init", "-b", "main"]);
        repo.git(&["config", "user.name", "Canopy Test"]);
        repo.git(&["config", "user.email", "canopy@example.com"]);
        repo
    }

    fn canopy(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_canopy"))
            .args(args)
            .current_dir(self.path())
            .output()
            .expect("run canopy")
    }

    fn git(&self, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.path())
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {:?} failed\nstdout:\n{}\nstderr:\n{}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git_output(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.path())
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("utf8 output")
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn write_file(&self, relative: &str, contents: &str) {
        std::fs::write(self.path().join(relative), contents).expect("write file")
    }
}
