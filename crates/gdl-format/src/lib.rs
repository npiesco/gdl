//! gdl-format: pure rendering helpers (status/diff `*_to_string`).

use std::{path::Path, sync::OnceLock};

use crossterm::style::{Color, ResetColor, SetForegroundColor};
use gdl_core::{ChangeKind, ChangeSection, DiffArea, FileDiff, GdlEntry, Hunk, Repository};
use serde::{Deserialize, Serialize};
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::SyntaxSet,
    util::as_24_bit_terminal_escaped,
};

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
    /// Render only repository-relative paths, one per line.
    PathsOnly,
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

/// Stable top-level JSON shape for diff output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffOutput {
    /// Schema version, tied to the `gdl-core` package version.
    pub version: String,
    /// Repository-relative file path.
    pub file: String,
    /// Diff area requested by the caller.
    pub area: DiffArea,
    /// Target terminal width propagated for deterministic round trips.
    pub width: usize,
    /// Whether either side is binary. Binary files never expose text hunks.
    pub binary: bool,
    /// Raw byte-preserving hunks.
    pub hunks: Vec<Hunk>,
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
        (OutputFormat::Plain | OutputFormat::Ansi, _, StatusView::PathsOnly) => {
            Ok(render_paths_only_status(&entries))
        }
        (OutputFormat::Json, _, StatusView::PathsOnly) => render_json_paths_only_status(&entries),
    }
}

/// Renders a repository file diff as a deterministic string.
pub fn diff_to_string(
    repo: &Repository,
    path: impl AsRef<Path>,
    options: &RenderOptions,
    area: DiffArea,
) -> Result<String, String> {
    let diff = gdl_core::diff(repo, path, area).map_err(|err| err.to_string())?;
    match (options.format, options.color) {
        (OutputFormat::Json, _) => render_json_diff(&diff, options.width),
        (OutputFormat::Plain, _) | (OutputFormat::Ansi, ColorPolicy::Never) => {
            Ok(render_plain_diff(&diff, options.width))
        }
        (OutputFormat::Ansi, ColorPolicy::Always) => Ok(render_ansi_diff(&diff, options.width)),
    }
}

fn render_json_status(entries: Vec<GdlEntry>) -> Result<String, String> {
    let output = StatusOutput {
        version: gdl_core::version().to_owned(),
        entries,
    };
    serde_json::to_string_pretty(&output).map_err(|err| err.to_string())
}

fn render_json_diff(diff: &FileDiff, width: usize) -> Result<String, String> {
    let output = DiffOutput {
        version: gdl_core::version().to_owned(),
        file: slash_path(&diff.file),
        area: diff.area,
        width,
        binary: diff.binary,
        hunks: diff.hunks.clone(),
    };
    serde_json::to_string_pretty(&output).map_err(|err| err.to_string())
}

fn render_plain_diff(diff: &FileDiff, width: usize) -> String {
    render_diff(diff, width, None)
}

fn render_ansi_diff(diff: &FileDiff, width: usize) -> String {
    render_diff(diff, width, Some(&ColorTheme::default()))
}

fn render_plain_status(entries: &[GdlEntry]) -> String {
    render_status(entries, None)
}

fn render_ansi_status(entries: &[GdlEntry]) -> String {
    render_status(entries, Some(&ColorTheme::default()))
}

fn render_paths_only_status(entries: &[GdlEntry]) -> String {
    let mut output = String::new();

    for entry in entries {
        output.push_str(&slash_path(&entry.path));
        output.push('\n');
    }

    output
}

fn render_json_paths_only_status(entries: &[GdlEntry]) -> Result<String, String> {
    let paths = entries
        .iter()
        .map(|entry| slash_path(&entry.path))
        .collect::<Vec<_>>();
    serde_json::to_string_pretty(&paths).map_err(|err| err.to_string())
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

fn render_diff(diff: &FileDiff, width: usize, theme: Option<&ColorTheme>) -> String {
    if diff.binary {
        return format!("Binary file {} changed\n", slash_path(&diff.file));
    }

    if width >= 120 {
        render_side_by_side_diff(diff, width, theme)
    } else {
        render_unified_diff(diff, theme)
    }
}

fn render_unified_diff(diff: &FileDiff, theme: Option<&ColorTheme>) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "diff --{} {}\n",
        diff.area,
        format_diff_path(&diff.file, theme)
    ));
    output.push_str(&format!("--- a/{}\n", slash_path(&diff.file)));
    output.push_str(&format!("+++ b/{}\n", slash_path(&diff.file)));

    for hunk in &diff.hunks {
        output.push_str(&format_hunk_header(hunk, theme));
        output.push('\n');
        for line in &hunk.old_bytes {
            push_diff_line(
                &mut output,
                "-",
                line,
                &diff.file,
                theme.map(|theme| theme.lines_removed),
                false,
            );
        }
        for line in &hunk.new_bytes {
            push_diff_line(
                &mut output,
                "+",
                line,
                &diff.file,
                theme.map(|theme| theme.lines_added),
                theme.is_some(),
            );
        }
    }

    output
}

fn render_side_by_side_diff(diff: &FileDiff, width: usize, theme: Option<&ColorTheme>) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "diff --{} {}\n",
        diff.area,
        format_diff_path(&diff.file, theme)
    ));

    let column_width = width.saturating_sub(5) / 2;
    for hunk in &diff.hunks {
        output.push_str(&format_hunk_header(hunk, theme));
        output.push('\n');

        let row_count = hunk.old_bytes.len().max(hunk.new_bytes.len());
        for index in 0..row_count {
            let old = hunk.old_bytes.get(index);
            let new = hunk.new_bytes.get(index);
            let old_cell = side_by_side_cell("-", old, &diff.file, column_width, theme, false);
            let new_cell = side_by_side_cell("+", new, &diff.file, column_width, theme, true);
            output.push_str(&old_cell);
            output.push_str(" | ");
            output.push_str(&new_cell);
            output.push('\n');
        }
    }

    output
}

fn side_by_side_cell(
    gutter: &str,
    line: Option<&Vec<u8>>,
    path: &Path,
    width: usize,
    theme: Option<&ColorTheme>,
    highlight: bool,
) -> String {
    let Some(line) = line else {
        return format!("{:<width$}", "");
    };

    let text = line_to_display(line)
        .trim_end_matches(['\r', '\n'])
        .to_owned();
    let plain = format!("{gutter}{text}");
    let padded = format!("{plain:<width$}");

    match theme {
        Some(theme) if highlight => {
            let gutter = paint(gutter, theme.lines_added);
            let highlighted = highlight_line(path, &text).unwrap_or(text);
            let padding = " ".repeat(padded.len().saturating_sub(plain.len()));
            format!("{gutter}{highlighted}{padding}")
        }
        Some(theme) => paint(&padded, theme.lines_removed),
        None => padded,
    }
}

fn format_diff_path(path: &Path, theme: Option<&ColorTheme>) -> String {
    let path = slash_path(path);
    match theme {
        Some(theme) => paint(&path, theme.filename),
        None => path,
    }
}

fn format_hunk_header(hunk: &Hunk, theme: Option<&ColorTheme>) -> String {
    maybe_paint(
        format!(
            "@@ -{} +{} @@",
            range_text(hunk.old_start, hunk.old_lines),
            range_text(hunk.new_start, hunk.new_lines)
        ),
        theme.map(|theme| theme.hunk_header),
    )
}

fn range_text(start: u32, lines: u32) -> String {
    if lines == 1 {
        start.to_string()
    } else {
        format!("{start},{lines}")
    }
}

fn push_diff_line(
    output: &mut String,
    gutter: &str,
    line: &[u8],
    path: &Path,
    color: Option<Color>,
    highlight: bool,
) {
    let text = line_to_display(line);

    if highlight {
        output.push_str(&maybe_paint(gutter.to_owned(), color));
        output.push_str(&highlight_line(path, &text).unwrap_or(text));
    } else {
        output.push_str(&maybe_paint(gutter.to_owned(), color));
        output.push_str(&maybe_paint(text, color));
    }

    if !line.ends_with(b"\n") {
        output.push('\n');
    }
}

fn line_to_display(line: &[u8]) -> String {
    String::from_utf8_lossy(line).into_owned()
}

fn highlight_line(path: &Path, line: &str) -> Option<String> {
    let syntax = syntax_set().find_syntax_for_file(path).ok().flatten()?;
    if syntax.name == "Plain Text" {
        return None;
    }

    let theme = syntax_theme()?;
    let mut highlighter = HighlightLines::new(syntax, theme);
    let ranges = highlighter.highlight_line(line, syntax_set()).ok()?;
    Some(as_24_bit_terminal_escaped(&ranges, false))
}

fn syntax_set() -> &'static SyntaxSet {
    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn syntax_theme() -> Option<&'static Theme> {
    static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();
    let themes = &THEME_SET.get_or_init(ThemeSet::load_defaults).themes;
    themes
        .get("base16-ocean.dark")
        .or_else(|| themes.values().next())
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
