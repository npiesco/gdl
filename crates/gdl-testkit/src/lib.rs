//! gdl-testkit: dev-only fixture builders for real git repositories.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Owns a temporary git repository used by integration tests.
#[derive(Debug)]
pub struct TestRepo {
    tempdir: tempfile::TempDir,
}

impl TestRepo {
    /// Initializes a real git repository on disk.
    pub fn init() -> Self {
        let tempdir = tempfile::tempdir().expect("tempdir must be created");
        let repo = Self { tempdir };

        repo.git(["init", "--initial-branch=main"]);
        repo.git(["config", "user.email", "gdl-test@example.com"]);
        repo.git(["config", "user.name", "GDL Test"]);

        repo
    }

    /// Returns the repository worktree path.
    pub fn path(&self) -> &Path {
        self.tempdir.path()
    }

    /// Returns a path inside the repository worktree.
    pub fn join(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.path().join(relative)
    }

    /// Writes bytes to a worktree path, creating parent directories as needed.
    pub fn write(&self, relative: impl AsRef<Path>, bytes: impl AsRef<[u8]>) {
        let path = self.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent directories must be created");
        }
        std::fs::write(path, bytes).expect("fixture file must be written");
    }

    /// Removes a worktree file.
    pub fn remove(&self, relative: impl AsRef<Path>) {
        std::fs::remove_file(self.join(relative)).expect("fixture file must be removed");
    }

    /// Runs a git command that must succeed.
    pub fn git<I, S>(&self, args: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.git_output(args);
        assert_git_success(&output);
    }

    /// Runs a git command and returns its raw output without asserting success.
    pub fn try_git<I, S>(&self, args: I) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.git_output(args)
    }

    fn git_output<I, S>(&self, args: I) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        Command::new("git")
            .args(args)
            .current_dir(self.path())
            .output()
            .expect("git command must start")
    }
}

fn assert_git_success(output: &Output) {
    assert!(
        output.status.success(),
        "git command failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
