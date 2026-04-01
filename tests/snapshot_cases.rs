use std::path::{Path, PathBuf};
use std::process::Command;

use insta::assert_snapshot;
use tempfile::TempDir;

#[test]
fn dedup_linear_snapshot() {
    assert_case_snapshot("dedup_linear");
}

#[test]
fn conflict_skip_snapshot() {
    assert_case_snapshot("conflict_skip");
}

fn assert_case_snapshot(case_name: &str) {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let case_script = repo_root
        .join("tests")
        .join("fixtures")
        .join(format!("{case_name}.sh"));
    let temp_dir = TempDir::new().expect("tempdir");
    let repo_dir = temp_dir.path().join("repo");
    std::fs::create_dir(&repo_dir).expect("create repo dir");

    let output = Command::new("zsh")
        .arg(case_script.as_os_str())
        .arg(env!("CARGO_BIN_EXE_canopy"))
        .current_dir(&repo_dir)
        .output()
        .expect("run snapshot case");

    assert!(
        output.status.success(),
        "snapshot case {case_name} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_path(snapshot_dir(&repo_root));
    settings.bind(|| {
        let snapshot = String::from_utf8(output.stdout).expect("snapshot output is utf8");
        assert_snapshot!(case_name, snapshot);
    });
}

fn snapshot_dir(repo_root: &Path) -> PathBuf {
    repo_root.join("tests").join("snapshots")
}
