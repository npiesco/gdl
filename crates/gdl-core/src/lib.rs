//! gdl-core: read-only git status + diff model.

use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

/// Returns the `gdl-core` package version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// A worktree-backed git repository opened by gdl.
#[derive(Debug)]
pub struct Repository {
    inner: gix::Repository,
}

impl Repository {
    /// Returns the `.git` directory for this repository.
    pub fn git_dir(&self) -> &Path {
        self.inner.git_dir()
    }

    /// Returns the root worktree directory for this repository.
    pub fn worktree_dir(&self) -> &Path {
        self.inner
            .workdir()
            .expect("Repository invariant: gdl rejects bare repositories at open time")
    }

    /// Returns the underlying gix repository for core operations.
    pub fn inner(&self) -> &gix::Repository {
        &self.inner
    }
}

/// Errors returned while opening a repository.
#[derive(Debug)]
pub enum OpenError {
    /// The requested path does not exist.
    NotFound { path: PathBuf },
    /// The path resolves to a bare repository, which gdl does not support in v0.1.
    BareRepository { git_dir: PathBuf },
    /// gix could not open or discover a repository from the requested path.
    Open { source: Box<gix::discover::Error> },
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::NotFound { path } => write!(f, "path does not exist: {}", path.display()),
            OpenError::BareRepository { git_dir } => {
                write!(
                    f,
                    "bare repositories are not supported: {}",
                    git_dir.display()
                )
            }
            OpenError::Open { source } => write!(f, "{source}"),
        }
    }
}

impl Error for OpenError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            OpenError::Open { source } => Some(source.as_ref()),
            OpenError::NotFound { .. } | OpenError::BareRepository { .. } => None,
        }
    }
}

/// Opens a worktree-backed git repository at `path`, discovering upward from
/// subdirectories when the literal path is not itself a repository root.
pub fn open(path: impl AsRef<Path>) -> Result<Repository, OpenError> {
    let path = path.as_ref();

    if !path.exists() {
        return Err(OpenError::NotFound {
            path: path.to_path_buf(),
        });
    }

    match gix::open(path.to_path_buf()) {
        Ok(repo) => Repository::from_gix(repo),
        Err(_) => gix::discover(path)
            .map_err(|source| OpenError::Open {
                source: Box::new(source),
            })
            .and_then(Repository::from_gix),
    }
}

impl Repository {
    fn from_gix(inner: gix::Repository) -> Result<Self, OpenError> {
        if inner.workdir().is_none() {
            return Err(OpenError::BareRepository {
                git_dir: inner.git_dir().to_path_buf(),
            });
        }

        Ok(Self { inner })
    }
}
