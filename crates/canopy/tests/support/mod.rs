use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

pub struct Fixture {
    dir: TempDir,
}

pub struct RemoteFixture {
    remote: TempDir,
    clone: TempDir,
}

impl Fixture {
    pub fn new() -> Self {
        let dir = TempDir::new().expect("tempdir");
        let fixture = Self { dir };
        fixture.git(&["init", "-b", "main"]);
        fixture.git(&["config", "user.name", "Canopy Test"]);
        fixture.git(&["config", "user.email", "canopy@example.com"]);
        fixture
    }

    pub fn with_base_commit(path: &str, contents: &str, message: &str) -> Self {
        let fixture = Self::new();
        fixture.commit_file(path, contents, message);
        fixture
    }

    pub fn sample_repo() -> Self {
        let fixture = Self::with_base_commit("story.txt", "base\n", "base");

        fixture.branch_from("linear", "main");
        fixture.amend_tracked_file("story.txt", "feature one\n", "feature one");
        fixture.git(&["branch", "linear-1"]);
        fixture.amend_tracked_file("story.txt", "feature two\n", "feature two");
        fixture.git(&["branch", "linear-2"]);

        fixture.branch_from("other", "main");
        fixture.commit_file("other.txt", "other\n", "other");

        fixture.branch_from("feature-a", "main");
        fixture.amend_tracked_file("story.txt", "feature a\n", "feature a");

        fixture.branch_from("feature-b", "main");
        fixture.amend_tracked_file("story.txt", "feature b\n", "feature b");

        fixture.branch_from("feature-c", "main");
        fixture.commit_file("extra.txt", "feature c\n", "feature c");

        fixture.checkout("main");
        fixture
    }

    pub fn canopy(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_canopy"))
            .args(args)
            .current_dir(self.path())
            .output()
            .expect("run canopy")
    }

    pub fn git(&self, args: &[&str]) {
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

    pub fn git_output(&self, args: &[&str]) -> String {
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

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn write_file(&self, relative: &str, contents: &str) {
        std::fs::write(self.path().join(relative), contents).expect("write file")
    }

    pub fn branch_from(&self, new_branch: &str, start_point: &str) {
        self.git(&["checkout", "-B", new_branch, start_point]);
    }

    pub fn checkout(&self, branch: &str) {
        self.git(&["checkout", branch]);
    }

    pub fn commit_file(&self, path: &str, contents: &str, message: &str) {
        self.write_file(path, contents);
        self.git(&["add", path]);
        self.git(&["commit", "-m", message]);
    }

    pub fn amend_tracked_file(&self, path: &str, contents: &str, message: &str) {
        self.write_file(path, contents);
        self.git(&["commit", "-am", message]);
    }
    pub fn assert_branch_file(&self, branch: &str, path: &str, expected: &str) {
        let spec = format!("{branch}:{path}");
        assert_eq!(self.git_output(&["show", spec.as_str()]), expected);
    }

    pub fn assert_branch_exists(&self, branch: &str) {
        self.git(&["rev-parse", "--verify", branch]);
    }
}

impl RemoteFixture {
    pub fn sample_repo() -> Self {
        let remote = TempDir::new().expect("remote tempdir");
        let remote_path = remote.path().join("origin.git");
        std::fs::create_dir(&remote_path).expect("create remote dir");
        git_in(
            remote.path(),
            &["init", "--bare", remote_path.to_str().expect("utf8 path")],
        );

        let seed = Fixture::sample_repo();
        seed.git(&[
            "remote",
            "add",
            "origin",
            remote_path.to_str().expect("utf8 path"),
        ]);
        seed.git(&["push", "--all", "origin"]);
        let clone = TempDir::new().expect("clone tempdir");
        git_in(
            clone.path(),
            &["clone", remote_path.to_str().expect("utf8 path"), "repo"],
        );

        let repo = Self { remote, clone };
        repo.git(&["config", "user.name", "Canopy Test"]);
        repo.git(&["config", "user.email", "canopy@example.com"]);
        repo
    }

    pub fn canopy(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_canopy"))
            .args(args)
            .current_dir(self.path())
            .output()
            .expect("run canopy")
    }

    pub fn git(&self, args: &[&str]) {
        git_in(&self.path(), args);
    }

    pub fn git_output(&self, args: &[&str]) -> String {
        git_output_in(&self.path(), args)
    }

    pub fn path(&self) -> PathBuf {
        self.clone.path().join("repo")
    }

    pub fn remote_path(&self) -> PathBuf {
        self.remote.path().join("origin.git")
    }

    pub fn assert_branch_file(&self, branch: &str, path: &str, expected: &str) {
        let spec = format!("{branch}:{path}");
        assert_eq!(self.git_output(&["show", spec.as_str()]), expected);
    }

    pub fn assert_remote_branch_exists(&self, branch: &str) {
        git_in(
            &self.remote_path(),
            &["rev-parse", "--verify", &format!("refs/heads/{branch}")],
        );
    }
}

fn git_in(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
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

fn git_output_in(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf8 output")
}
