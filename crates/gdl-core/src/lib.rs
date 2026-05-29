//! gdl-core: read-only git status + diff model.

use std::borrow::Cow;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

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

/// Logical status section for an entry, matching the VS Code Source Control
/// grouping model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeSection {
    /// Changes staged in the index relative to `HEAD`.
    Staged,
    /// Tracked worktree changes relative to the index.
    WorkingTree,
    /// Untracked worktree paths.
    Untracked,
    /// Paths with unresolved index conflicts.
    Conflicted,
}

/// Git change kind normalized for gdl renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeKind {
    /// Existing path contents changed.
    Modified,
    /// New tracked path.
    Added,
    /// Tracked path removed.
    Deleted,
    /// Path renamed from `old_path` to `path`.
    Renamed,
    /// Path copied from `old_path` to `path`.
    Copied,
    /// Worktree path not yet tracked by the index.
    Untracked,
    /// Unresolved merge conflict.
    Conflicted,
    /// File type changed.
    TypeChanged,
}

/// One normalized git status entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GdlEntry {
    /// Which Source Control section owns this entry.
    pub section: ChangeSection,
    /// Normalized change kind.
    pub kind: ChangeKind,
    /// Repository-relative current path.
    pub path: PathBuf,
    /// Repository-relative previous path for renames/copies.
    pub old_path: Option<PathBuf>,
    /// Added text lines; zero for binary/conflicted entries.
    pub lines_added: usize,
    /// Removed text lines; zero for binary/conflicted entries.
    pub lines_removed: usize,
    /// Whether at least one side of the content comparison is binary.
    pub is_binary: bool,
}

/// Which pair of repository states to diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffArea {
    /// Worktree bytes compared to the index.
    Worktree,
    /// Index bytes compared to `HEAD`.
    Staged,
    /// Worktree bytes compared to `HEAD`.
    Head,
}

impl fmt::Display for DiffArea {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            DiffArea::Worktree => "worktree",
            DiffArea::Staged => "staged",
            DiffArea::Head => "head",
        })
    }
}

/// One raw byte-preserving diff hunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hunk {
    /// 1-based start line in the old side, or 0 for a pure addition at file start.
    pub old_start: u32,
    /// Number of old-side changed lines.
    pub old_lines: u32,
    /// 1-based start line in the new side, or 0 for a pure deletion at file start.
    pub new_start: u32,
    /// Number of new-side changed lines.
    pub new_lines: u32,
    /// Raw removed line bytes, including original line endings.
    pub old_bytes: Vec<Vec<u8>>,
    /// Raw added line bytes, including original line endings.
    pub new_bytes: Vec<Vec<u8>>,
}

/// Raw diff data for a single repository-relative file path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDiff {
    /// Repository-relative file path.
    pub file: PathBuf,
    /// Diff area used to choose old and new bytes.
    pub area: DiffArea,
    /// Whether either side is binary. Binary files do not expose text hunks.
    pub binary: bool,
    /// Text hunks in gix-diff order.
    pub hunks: Vec<Hunk>,
}

/// Errors returned while collecting repository status.
#[derive(Debug)]
pub struct StatusError {
    message: String,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl StatusError {
    fn with_source(message: impl Into<String>, source: impl Error + Send + Sync + 'static) -> Self {
        Self {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
}

impl fmt::Display for StatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for StatusError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

/// Errors returned while collecting a file diff.
#[derive(Debug)]
pub struct DiffError {
    message: String,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl DiffError {
    fn simple(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source: None,
        }
    }

    fn with_source(message: impl Into<String>, source: impl Error + Send + Sync + 'static) -> Self {
        Self {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
}

impl fmt::Display for DiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for DiffError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

/// Returns normalized repository status entries.
pub fn status(repo: &Repository) -> Result<Vec<GdlEntry>, StatusError> {
    let mut entries = Vec::new();
    let iter = repo
        .inner()
        .status(gix::progress::Discard)
        .map_err(|err| StatusError::with_source("failed to create status platform", err))?
        .untracked_files(gix::status::UntrackedFiles::Files)
        .tree_index_track_renames(gix::status::tree_index::TrackRenames::AsConfigured)
        .into_iter(Vec::<gix::bstr::BString>::new())
        .map_err(|err| StatusError::with_source("failed to create status iterator", err))?;

    for item in iter {
        let item =
            item.map_err(|err| StatusError::with_source("failed to read status item", err))?;
        match item {
            gix::status::Item::TreeIndex(change) => {
                push_tree_index_entry(repo, change, &mut entries)?;
            }
            gix::status::Item::IndexWorktree(item) => {
                push_index_worktree_entry(repo, item, &mut entries)?;
            }
        }
    }

    entries.sort_by(|left, right| {
        section_order(left.section)
            .cmp(&section_order(right.section))
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| kind_order(left.kind).cmp(&kind_order(right.kind)))
    });

    Ok(entries)
}

/// Returns raw hunks for `path` in the requested diff area.
pub fn diff(
    repo: &Repository,
    path: impl AsRef<Path>,
    area: DiffArea,
) -> Result<FileDiff, DiffError> {
    let file = repo_relative_path(path.as_ref())?;
    let (old, new) = diff_sides(repo, &file, area)?;
    let binary = is_binary(&old) || is_binary(&new);
    let hunks = if binary {
        Vec::new()
    } else {
        diff_hunks(&old, &new)
    };

    Ok(FileDiff {
        file,
        area,
        binary,
        hunks,
    })
}

fn push_tree_index_entry(
    repo: &Repository,
    change: gix_diff::index::Change,
    entries: &mut Vec<GdlEntry>,
) -> Result<(), StatusError> {
    use gix_diff::index::ChangeRef;

    match change {
        ChangeRef::Addition { location, id, .. } => {
            let new = blob_bytes(repo, id)?;
            let (lines_added, lines_removed, is_binary) = diff_counts(&[], &new);
            entries.push(GdlEntry {
                section: ChangeSection::Staged,
                kind: ChangeKind::Added,
                path: path_from_bstr(location.as_ref()),
                old_path: None,
                lines_added,
                lines_removed,
                is_binary,
            });
        }
        ChangeRef::Deletion { location, id, .. } => {
            let old = blob_bytes(repo, id)?;
            let (lines_added, lines_removed, is_binary) = diff_counts(&old, &[]);
            entries.push(GdlEntry {
                section: ChangeSection::Staged,
                kind: ChangeKind::Deleted,
                path: path_from_bstr(location.as_ref()),
                old_path: None,
                lines_added,
                lines_removed,
                is_binary,
            });
        }
        ChangeRef::Modification {
            location,
            previous_id,
            id,
            ..
        } => {
            let old = blob_bytes(repo, previous_id)?;
            let new = blob_bytes(repo, id)?;
            let (lines_added, lines_removed, is_binary) = diff_counts(&old, &new);
            entries.push(GdlEntry {
                section: ChangeSection::Staged,
                kind: ChangeKind::Modified,
                path: path_from_bstr(location.as_ref()),
                old_path: None,
                lines_added,
                lines_removed,
                is_binary,
            });
        }
        ChangeRef::Rewrite {
            source_location,
            source_id,
            location,
            id,
            copy,
            ..
        } => {
            let old = blob_bytes(repo, source_id)?;
            let new = blob_bytes(repo, id)?;
            let (lines_added, lines_removed, is_binary) = diff_counts(&old, &new);
            entries.push(GdlEntry {
                section: ChangeSection::Staged,
                kind: if copy {
                    ChangeKind::Copied
                } else {
                    ChangeKind::Renamed
                },
                path: path_from_bstr(location.as_ref()),
                old_path: Some(path_from_bstr(source_location.as_ref())),
                lines_added,
                lines_removed,
                is_binary,
            });
        }
    }

    Ok(())
}

fn push_index_worktree_entry(
    repo: &Repository,
    item: gix::status::index_worktree::Item,
    entries: &mut Vec<GdlEntry>,
) -> Result<(), StatusError> {
    use gix::status::plumbing::index_as_worktree::{Change, EntryStatus};

    match item {
        gix::status::index_worktree::Item::Modification {
            entry,
            rela_path,
            status,
            ..
        } => match status {
            EntryStatus::Conflict { .. } => entries.push(GdlEntry {
                section: ChangeSection::Conflicted,
                kind: ChangeKind::Conflicted,
                path: path_from_bstr(rela_path),
                old_path: None,
                lines_added: 0,
                lines_removed: 0,
                is_binary: false,
            }),
            EntryStatus::Change(Change::Removed) => {
                let old = blob_bytes(repo, Cow::Owned(entry.id))?;
                let (lines_added, lines_removed, is_binary) = diff_counts(&old, &[]);
                entries.push(GdlEntry {
                    section: ChangeSection::WorkingTree,
                    kind: ChangeKind::Deleted,
                    path: path_from_bstr(rela_path),
                    old_path: None,
                    lines_added,
                    lines_removed,
                    is_binary,
                });
            }
            EntryStatus::Change(Change::Modification { .. })
            | EntryStatus::Change(Change::SubmoduleModification(_)) => {
                let old = blob_bytes(repo, Cow::Owned(entry.id))?;
                let new = worktree_bytes(repo, path_from_bstr(&rela_path))?;
                let (lines_added, lines_removed, is_binary) = diff_counts(&old, &new);
                entries.push(GdlEntry {
                    section: ChangeSection::WorkingTree,
                    kind: ChangeKind::Modified,
                    path: path_from_bstr(rela_path),
                    old_path: None,
                    lines_added,
                    lines_removed,
                    is_binary,
                });
            }
            EntryStatus::Change(Change::Type { .. }) => {
                let old = blob_bytes(repo, Cow::Owned(entry.id))?;
                let new = worktree_bytes(repo, path_from_bstr(&rela_path)).unwrap_or_default();
                let (lines_added, lines_removed, is_binary) = diff_counts(&old, &new);
                entries.push(GdlEntry {
                    section: ChangeSection::WorkingTree,
                    kind: ChangeKind::TypeChanged,
                    path: path_from_bstr(rela_path),
                    old_path: None,
                    lines_added,
                    lines_removed,
                    is_binary,
                });
            }
            EntryStatus::IntentToAdd | EntryStatus::NeedsUpdate(_) => {}
        },
        gix::status::index_worktree::Item::DirectoryContents { entry, .. } => {
            if matches!(entry.status, gix::dir::entry::Status::Untracked) {
                let path = path_from_bstr(entry.rela_path);
                let new = worktree_bytes(repo, &path)?;
                let (lines_added, lines_removed, is_binary) = diff_counts(&[], &new);
                entries.push(GdlEntry {
                    section: ChangeSection::Untracked,
                    kind: ChangeKind::Untracked,
                    path,
                    old_path: None,
                    lines_added,
                    lines_removed,
                    is_binary,
                });
            }
        }
        gix::status::index_worktree::Item::Rewrite {
            source,
            dirwalk_entry,
            copy,
            ..
        } => {
            let old_path = path_from_bstr(source.rela_path());
            let path = path_from_bstr(dirwalk_entry.rela_path);
            let old = match source {
                gix::status::index_worktree::RewriteSource::RewriteFromIndex {
                    source_entry,
                    ..
                } => blob_bytes(repo, Cow::Owned(source_entry.id))?,
                gix::status::index_worktree::RewriteSource::CopyFromDirectoryEntry { .. } => {
                    worktree_bytes(repo, &old_path)?
                }
            };
            let new = worktree_bytes(repo, &path)?;
            let (lines_added, lines_removed, is_binary) = diff_counts(&old, &new);
            entries.push(GdlEntry {
                section: ChangeSection::WorkingTree,
                kind: if copy {
                    ChangeKind::Copied
                } else {
                    ChangeKind::Renamed
                },
                path,
                old_path: Some(old_path),
                lines_added,
                lines_removed,
                is_binary,
            });
        }
    }

    Ok(())
}

fn blob_bytes(repo: &Repository, id: Cow<'_, gix::hash::oid>) -> Result<Vec<u8>, StatusError> {
    repo.inner()
        .find_blob(id.into_owned())
        .map(|blob| blob.data.clone())
        .map_err(|err| StatusError::with_source("failed to read blob", err))
}

fn worktree_bytes(repo: &Repository, path: impl AsRef<Path>) -> Result<Vec<u8>, StatusError> {
    fs::read(repo.worktree_dir().join(path.as_ref()))
        .map_err(|err| StatusError::with_source("failed to read worktree file", err))
}

fn diff_sides(
    repo: &Repository,
    path: &Path,
    area: DiffArea,
) -> Result<(Vec<u8>, Vec<u8>), DiffError> {
    match area {
        DiffArea::Worktree => Ok((
            index_blob_bytes(repo, path)?.unwrap_or_default(),
            worktree_bytes_optional(repo, path)?.unwrap_or_default(),
        )),
        DiffArea::Staged => Ok((
            head_blob_bytes(repo, path)?.unwrap_or_default(),
            index_blob_bytes(repo, path)?.unwrap_or_default(),
        )),
        DiffArea::Head => Ok((
            head_blob_bytes(repo, path)?.unwrap_or_default(),
            worktree_bytes_optional(repo, path)?.unwrap_or_default(),
        )),
    }
}

fn diff_hunks(old: &[u8], new: &[u8]) -> Vec<Hunk> {
    let old_lines = gix_diff::blob::sources::byte_lines(old).collect::<Vec<_>>();
    let new_lines = gix_diff::blob::sources::byte_lines(new).collect::<Vec<_>>();
    let input = gix_diff::blob::InternedInput::new(
        gix_diff::blob::sources::byte_lines(old),
        gix_diff::blob::sources::byte_lines(new),
    );
    let diff =
        gix_diff::blob::diff_with_slider_heuristics(gix_diff::blob::Algorithm::Histogram, &input);

    diff.hunks()
        .map(|hunk| Hunk {
            old_start: range_start(hunk.before.start, hunk.before.end),
            old_lines: hunk.before.end - hunk.before.start,
            new_start: range_start(hunk.after.start, hunk.after.end),
            new_lines: hunk.after.end - hunk.after.start,
            old_bytes: old_lines[hunk.before.start as usize..hunk.before.end as usize]
                .iter()
                .map(|line| line.to_vec())
                .collect(),
            new_bytes: new_lines[hunk.after.start as usize..hunk.after.end as usize]
                .iter()
                .map(|line| line.to_vec())
                .collect(),
        })
        .collect()
}

fn range_start(start: u32, end: u32) -> u32 {
    if start == 0 && end == 0 {
        0
    } else {
        start + 1
    }
}

fn head_blob_bytes(repo: &Repository, path: &Path) -> Result<Option<Vec<u8>>, DiffError> {
    let tree_id = repo
        .inner()
        .head_tree_id_or_empty()
        .map_err(|err| DiffError::with_source("failed to resolve HEAD tree", err))?;
    let mut tree = repo
        .inner()
        .find_tree(tree_id)
        .map_err(|err| DiffError::with_source("failed to read HEAD tree", err))?;
    let Some(entry) = tree
        .peel_to_entry_by_path(path)
        .map_err(|err| DiffError::with_source("failed to look up path in HEAD", err))?
    else {
        return Ok(None);
    };

    diff_blob_bytes(repo, entry.object_id()).map(Some)
}

fn index_blob_bytes(repo: &Repository, path: &Path) -> Result<Option<Vec<u8>>, DiffError> {
    if !repo.inner().index_path().exists() {
        return Ok(None);
    }

    let index = repo
        .inner()
        .open_index()
        .map_err(|err| DiffError::with_source("failed to open index", err))?;
    let path = slash_path(path);
    let Some(entry) = index.entry_by_path(gix::bstr::ByteSlice::as_bstr(path.as_bytes())) else {
        return Ok(None);
    };

    diff_blob_bytes(repo, entry.id).map(Some)
}

fn worktree_bytes_optional(repo: &Repository, path: &Path) -> Result<Option<Vec<u8>>, DiffError> {
    match fs::read(repo.worktree_dir().join(path)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(DiffError::with_source("failed to read worktree file", err)),
    }
}

fn diff_blob_bytes(repo: &Repository, id: gix::ObjectId) -> Result<Vec<u8>, DiffError> {
    repo.inner()
        .find_blob(id)
        .map(|blob| blob.data.clone())
        .map_err(|err| DiffError::with_source("failed to read blob", err))
}

fn diff_counts(old: &[u8], new: &[u8]) -> (usize, usize, bool) {
    let is_binary = is_binary(old) || is_binary(new);
    if is_binary {
        return (0, 0, true);
    }

    let old_lines = byte_lines(old);
    let new_lines = byte_lines(new);
    let common = lcs_len(&old_lines, &new_lines);

    (new_lines.len() - common, old_lines.len() - common, false)
}

fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8 * 1024).any(|byte| *byte == 0)
}

fn byte_lines(bytes: &[u8]) -> Vec<&[u8]> {
    if bytes.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut start = 0;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            lines.push(&bytes[start..=index]);
            start = index + 1;
        }
    }
    if start < bytes.len() {
        lines.push(&bytes[start..]);
    }

    lines
}

fn lcs_len(left: &[&[u8]], right: &[&[u8]]) -> usize {
    let mut previous = vec![0; right.len() + 1];
    let mut current = vec![0; right.len() + 1];

    for left_line in left {
        for (right_index, right_line) in right.iter().enumerate() {
            current[right_index + 1] = if left_line == right_line {
                previous[right_index] + 1
            } else {
                previous[right_index + 1].max(current[right_index])
            };
        }
        std::mem::swap(&mut previous, &mut current);
        current.fill(0);
    }

    previous[right.len()]
}

fn path_from_bstr(bytes: impl AsRef<[u8]>) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes.as_ref()).into_owned())
}

fn repo_relative_path(path: &Path) -> Result<PathBuf, DiffError> {
    if path.as_os_str().is_empty() {
        return Err(DiffError::simple("diff path must not be empty"));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(DiffError::simple(format!(
                    "diff path must be repository-relative: {}",
                    path.display()
                )));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(DiffError::simple("diff path must not be empty"));
    }

    Ok(normalized)
}

fn slash_path(path: &Path) -> String {
    path.iter()
        .map(|component| component.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn section_order(section: ChangeSection) -> u8 {
    match section {
        ChangeSection::Staged => 0,
        ChangeSection::WorkingTree => 1,
        ChangeSection::Untracked => 2,
        ChangeSection::Conflicted => 3,
    }
}

fn kind_order(kind: ChangeKind) -> u8 {
    match kind {
        ChangeKind::Added => 0,
        ChangeKind::Modified => 1,
        ChangeKind::Deleted => 2,
        ChangeKind::Renamed => 3,
        ChangeKind::Copied => 4,
        ChangeKind::TypeChanged => 5,
        ChangeKind::Untracked => 6,
        ChangeKind::Conflicted => 7,
    }
}
