//! gdl-format: pure rendering helpers (status/diff `*_to_string`).

use std::path::Path;

use crossterm::style::{Color, ResetColor, SetForegroundColor};
use gdl_core::{ChangeKind, ChangeSection, GdlEntry, Repository};
use serde::{Deserialize, Serialize};

/// Output encoding for format renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Deterministic plain text without ANSI escapes.
    Plain,
    /// Deterministic plain text with ANSI escapes.
    Ansi,
    /// Stable JSON for tools and agent consumers.
    Json,
}

/// ANSI color policy for renderers that support color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorPolicy {
    /// Never emit ANSI escapes.
    Never,
    /// Always emit ANSI escapes.
    Always,
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

/// Color slots used by ANSI status and diff renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorTheme {
    pub modified: Color,
    pub added: Color,
    pub deleted: Color,
    pub renamed: Color,
    pub conflict: Color,
    pub untracked: Color,
    pub filename: Color,
    pub dir_path: Color,
    pub lines_added: Color,
    pub lines_removed: Color,
    pub hunk_header: Color,
    pub section_header: Color,
}

impl Default for ColorTheme {
    fn default() -> Self {
        Self {
            modified: Color::Yellow,
            added: Color::Green,
            deleted: Color::Red,
            renamed: Color::Cyan,
            conflict: Color::Red,
            untracked: Color::Blue,
            filename: Color::White,
            dir_path: Color::DarkGrey,
            lines_added: Color::Green,
            lines_removed: Color::Red,
            hunk_header: Color::Magenta,
            section_header: Color::Cyan,
        }
    }
}

/// Stable top-level JSON shape for status output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusOutput {
    /// Schema version, tied to the `gdl-core` package version.
    pub version: String,
    /// Status entries in core status order.
    pub entries: Vec<GdlEntry>,
}

/// Renders repository status as a deterministic string.
pub fn status_to_string(repo: &Repository, options: &RenderOptions) -> Result<String, String> {
    let _width = options.width;

    let entries = gdl_core::status(repo).map_err(|err| err.to_string())?;
    match (options.format, options.color, options.view) {
        (OutputFormat::Plain, _, StatusView::Full)
        | (OutputFormat::Ansi, ColorPolicy::Never, StatusView::Full) => {
            Ok(render_plain_status(&entries))
        }
        (OutputFormat::Ansi, ColorPolicy::Always, StatusView::Full) => {
            Ok(render_ansi_status(&entries))
        }
        (OutputFormat::Json, _, StatusView::Full) => render_json_status(entries),
    }
}

fn render_json_status(entries: Vec<GdlEntry>) -> Result<String, String> {
    let output = StatusOutput {
        version: gdl_core::version().to_owned(),
        entries,
    };
    serde_json::to_string_pretty(&output).map_err(|err| err.to_string())
}

fn render_plain_status(entries: &[GdlEntry]) -> String {
    render_status(entries, None)
}

fn render_ansi_status(entries: &[GdlEntry]) -> String {
    render_status(entries, Some(&ColorTheme::default()))
}

fn render_status(entries: &[GdlEntry], theme: Option<&ColorTheme>) -> String {
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

        let header = format!("{} ({})", section_title(section), section_entries.len());
        output.push_str(&format_section_header(&header, theme));
        output.push('\n');

        for entry in section_entries {
            output.push_str(&format_status_entry(entry, theme));
            output.push('\n');
        }
    }

    output
}

fn format_section_header(header: &str, theme: Option<&ColorTheme>) -> String {
    match theme {
        Some(theme) => paint(header, theme.section_header),
        None => header.to_owned(),
    }
}

fn format_status_entry(entry: &GdlEntry, theme: Option<&ColorTheme>) -> String {
    let badge = kind_badge(entry.kind);
    let filename = file_name(&entry.path);
    let directory = directory_name(&entry.path);
    let filename_field = format!("{filename:<17}");
    let directory_field = format!("{directory:<7}");
    let mut line = format!(
        "{}  {} {} {}",
        format_badge(badge, entry.kind, theme),
        format_filename_field(&filename_field, theme),
        format_directory_field(&directory_field, theme),
        format_count_text(entry, theme)
    );

    if let Some(old_path) = &entry.old_path {
        line.push_str("  from ");
        line.push_str(&format_directory_field(&slash_path(old_path), theme));
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

fn format_badge(badge: &str, kind: ChangeKind, theme: Option<&ColorTheme>) -> String {
    match theme {
        Some(theme) => paint(badge, kind_color(kind, theme)),
        None => badge.to_owned(),
    }
}

fn format_filename_field(filename: &str, theme: Option<&ColorTheme>) -> String {
    match theme {
        Some(theme) => paint(filename, theme.filename),
        None => filename.to_owned(),
    }
}

fn format_directory_field(directory: &str, theme: Option<&ColorTheme>) -> String {
    match theme {
        Some(theme) => paint(directory, theme.dir_path),
        None => directory.to_owned(),
    }
}

fn format_count_text(entry: &GdlEntry, theme: Option<&ColorTheme>) -> String {
    if entry.kind == ChangeKind::Conflicted {
        maybe_paint("!".to_owned(), theme.map(|theme| theme.conflict))
    } else if entry.is_binary {
        maybe_paint("binary".to_owned(), theme.map(|theme| theme.lines_added))
    } else {
        let added = format!("+{}", entry.lines_added);
        let removed = format!("-{}", entry.lines_removed);
        match theme {
            Some(theme) => format!(
                "{} {}",
                paint(&added, theme.lines_added),
                paint(&removed, theme.lines_removed)
            ),
            None => format!("{added} {removed}"),
        }
    }
}

fn kind_color(kind: ChangeKind, theme: &ColorTheme) -> Color {
    match kind {
        ChangeKind::Modified | ChangeKind::TypeChanged => theme.modified,
        ChangeKind::Added => theme.added,
        ChangeKind::Deleted => theme.deleted,
        ChangeKind::Renamed | ChangeKind::Copied => theme.renamed,
        ChangeKind::Untracked => theme.untracked,
        ChangeKind::Conflicted => theme.conflict,
    }
}

fn maybe_paint(text: String, color: Option<Color>) -> String {
    match color {
        Some(color) => paint(&text, color),
        None => text,
    }
}

fn paint(text: &str, color: Color) -> String {
    format!("{}{}{}", SetForegroundColor(color), text, ResetColor)
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
