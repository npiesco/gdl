//! gdl-format: pure rendering helpers (status/diff `*_to_string`).

use std::path::Path;

use gdl_core::{ChangeKind, ChangeSection, GdlEntry, Repository};

/// Output encoding for format renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Deterministic plain text without ANSI escapes.
    Plain,
}

/// ANSI color policy for renderers that support color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorPolicy {
    /// Never emit ANSI escapes.
    Never,
}

/// Status rendering mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusView {
    /// Render all status sections.
    Full,
}

/// Rendering options shared by status renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOptions {
    /// Requested output format.
    pub format: OutputFormat,
    /// Requested ANSI color policy.
    pub color: ColorPolicy,
    /// Target terminal width.
    pub width: usize,
    /// Requested status view.
    pub view: StatusView,
}

/// Renders repository status as a deterministic string.
pub fn status_to_string(repo: &Repository, options: &RenderOptions) -> Result<String, String> {
    match (options.format, options.color, options.view) {
        (OutputFormat::Plain, ColorPolicy::Never, StatusView::Full) => {}
    }
    let _width = options.width;

    let entries = gdl_core::status(repo).map_err(|err| err.to_string())?;
    Ok(render_plain_status(&entries))
}

fn render_plain_status(entries: &[GdlEntry]) -> String {
    let mut output = String::new();
    let mut is_first_section = true;

    for section in [
        ChangeSection::Staged,
        ChangeSection::WorkingTree,
        ChangeSection::Untracked,
        ChangeSection::Conflicted,
    ] {
        let mut section_entries = entries
            .iter()
            .filter(|entry| entry.section == section)
            .collect::<Vec<_>>();
        if section_entries.is_empty() {
            continue;
        }
        section_entries.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| kind_order(left.kind).cmp(&kind_order(right.kind)))
        });

        if !is_first_section {
            output.push('\n');
        }
        is_first_section = false;

        output.push_str(section_title(section));
        output.push_str(" (");
        output.push_str(&section_entries.len().to_string());
        output.push_str(")\n");

        for entry in section_entries {
            output.push_str(&format_plain_entry(entry));
            output.push('\n');
        }
    }

    output
}

fn format_plain_entry(entry: &GdlEntry) -> String {
    let filename = file_name(&entry.path);
    let directory = directory_name(&entry.path);
    let mut line = format!(
        "{}  {:<17} {:<7} {}",
        kind_badge(entry.kind),
        filename,
        directory,
        count_text(entry)
    );

    if let Some(old_path) = &entry.old_path {
        line.push_str("  from ");
        line.push_str(&slash_path(old_path));
    }

    line
}

fn section_title(section: ChangeSection) -> &'static str {
    match section {
        ChangeSection::Staged => "Staged Changes",
        ChangeSection::WorkingTree => "Changes",
        ChangeSection::Untracked => "Untracked",
        ChangeSection::Conflicted => "Merge Changes",
    }
}

fn kind_badge(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Modified => "M",
        ChangeKind::Added => "A",
        ChangeKind::Deleted => "D",
        ChangeKind::Renamed => "R",
        ChangeKind::Copied => "C",
        ChangeKind::Untracked => "U",
        ChangeKind::Conflicted => "!",
        ChangeKind::TypeChanged => "T",
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

fn count_text(entry: &GdlEntry) -> String {
    if entry.kind == ChangeKind::Conflicted {
        "!".to_owned()
    } else if entry.is_binary {
        "binary".to_owned()
    } else {
        format!("+{} -{}", entry.lines_added, entry.lines_removed)
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|component| component.to_string_lossy().into_owned())
        .unwrap_or_else(|| slash_path(path))
}

fn directory_name(path: &Path) -> String {
    match path.parent() {
        Some(parent) if parent != Path::new("") => slash_path(parent),
        _ => ".".to_owned(),
    }
}

fn slash_path(path: &Path) -> String {
    if path.as_os_str().is_empty() {
        return ".".to_owned();
    }

    path.iter()
        .map(|component| component.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
