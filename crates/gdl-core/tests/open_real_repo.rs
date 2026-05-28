use std::path::Path;
use std::process::Command;

fn git(args: &[&str], cwd: &Path) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git command must start");

    assert!(
        output.status.success(),
        "git {args:?} failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn open_opens_repo_at_exact_path() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = tempfile::tempdir()?;
    git(&["init", "--initial-branch=main"], tmp.path());

    let repo = gdl_core::open(tmp.path())?;

    assert_eq!(repo.worktree_dir(), tmp.path());
    assert_eq!(repo.git_dir(), tmp.path().join(".git"));

    Ok(())
}

#[test]
fn open_discovers_repo_from_subdirectory() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = tempfile::tempdir()?;
    git(&["init", "--initial-branch=main"], tmp.path());

    let nested = tmp.path().join("a").join("b");
    std::fs::create_dir_all(&nested)?;

    let repo = gdl_core::open(&nested)?;

    assert_eq!(repo.worktree_dir(), tmp.path());
    assert_eq!(repo.git_dir(), tmp.path().join(".git"));

    Ok(())
}

#[test]
fn open_missing_path_returns_typed_error() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = tempfile::tempdir()?;
    let missing = tmp.path().join("missing");

    let err = match gdl_core::open(&missing) {
        Ok(_) => panic!("expected missing path to return OpenError::NotFound"),
        Err(err) => err,
    };

    match err {
        gdl_core::OpenError::NotFound { path } => assert_eq!(path, missing),
        other => panic!("expected OpenError::NotFound, got {other:?}"),
    }

    Ok(())
}
