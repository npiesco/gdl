use crossterm::style::SetForegroundColor;
use gdl_core::DiffArea;
use gdl_format::{
    diff_to_string, status_to_string, ColorPolicy, ColorTheme, OutputFormat, RenderOptions,
    StatusView,
};
use gdl_testkit::TestRepo;
use syntect::{
    easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet,
    util::as_24_bit_terminal_escaped,
};

#[test]
fn status_ansi_strips_to_plain_and_colors_status_badges() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = status_fixture(["nested/modified.txt", "staged.txt", "untracked.txt"]);
    let repo = gdl_core::open(fixture.path())?;

    let plain = status_to_string(&repo, &plain_options())?;
    let ansi = status_to_string(&repo, &ansi_options())?;

    assert_eq!(strip_ansi_escapes::strip(ansi.as_bytes()), plain.as_bytes());

    let theme = ColorTheme::default();
    assert_token_color(
        &ansi,
        "Staged Changes (2)",
        SetForegroundColor(theme.section_header),
    );
    assert_badge_color(
        &ansi,
        'M',
        "modified.txt",
        SetForegroundColor(theme.modified),
    );
    assert_badge_color(&ansi, 'A', "staged.txt", SetForegroundColor(theme.added));
    assert_badge_color(&ansi, 'D', "deleted.txt", SetForegroundColor(theme.deleted));
    assert_badge_color(&ansi, 'R', "renamed.txt", SetForegroundColor(theme.renamed));
    assert_badge_color(
        &ansi,
        'U',
        "untracked.txt",
        SetForegroundColor(theme.untracked),
    );
    assert_token_after_filename_color(
        &ansi,
        "modified.txt",
        "+1",
        SetForegroundColor(theme.lines_added),
    );
    assert_token_after_filename_color(
        &ansi,
        "deleted.txt",
        "-1",
        SetForegroundColor(theme.lines_removed),
    );

    Ok(())
}

#[test]
fn status_ansi_colors_conflicted_status() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = conflicted_fixture();
    let repo = gdl_core::open(fixture.path())?;

    let plain = status_to_string(&repo, &plain_options())?;
    let ansi = status_to_string(&repo, &ansi_options())?;

    assert_eq!(strip_ansi_escapes::strip(ansi.as_bytes()), plain.as_bytes());

    let theme = ColorTheme::default();
    assert_badge_color(
        &ansi,
        '!',
        "conflict.txt",
        SetForegroundColor(theme.conflict),
    );
    assert_token_after_filename_color(
        &ansi,
        "conflict.txt",
        "!",
        SetForegroundColor(theme.conflict),
    );

    Ok(())
}

#[test]
fn status_ansi_ignores_no_color_environment() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = status_fixture(["nested/modified.txt", "staged.txt", "untracked.txt"]);
    let repo = gdl_core::open(fixture.path())?;

    let previous_no_color = std::env::var_os("NO_COLOR");
    std::env::remove_var("NO_COLOR");
    let without_no_color = status_to_string(&repo, &ansi_options())?;

    std::env::set_var("NO_COLOR", "1");
    let with_no_color = status_to_string(&repo, &ansi_options())?;

    match previous_no_color {
        Some(value) => std::env::set_var("NO_COLOR", value),
        None => std::env::remove_var("NO_COLOR"),
    }

    assert_eq!(with_no_color, without_no_color);
    assert_ne!(with_no_color, status_to_string(&repo, &plain_options())?);

    Ok(())
}

#[test]
fn diff_ansi_strips_to_plain_and_runs_syntect() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = diff_fixture("src/app.rs");
    let repo = gdl_core::open(fixture.path())?;

    let plain = diff_to_string(
        &repo,
        "src/app.rs",
        &plain_diff_options(80),
        DiffArea::Worktree,
    )?;
    let ansi = diff_to_string(
        &repo,
        "src/app.rs",
        &ansi_diff_options(80),
        DiffArea::Worktree,
    )?;

    assert_eq!(strip_ansi_escapes::strip(ansi.as_bytes()), plain.as_bytes());
    assert!(ansi.contains(&syntect_oracle_line("src/app.rs")?));

    Ok(())
}

#[test]
fn diff_ansi_plain_text_fallback_keeps_only_gutter_color() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = diff_fixture("notes.txt");
    let repo = gdl_core::open(fixture.path())?;

    let ansi = diff_to_string(
        &repo,
        "notes.txt",
        &ansi_diff_options(80),
        DiffArea::Worktree,
    )?;

    assert!(!ansi.contains("\x1b[38;2;"));
    assert!(ansi.contains(&SetForegroundColor(ColorTheme::default().lines_added).to_string()));

    Ok(())
}

#[test]
fn diff_ansi_uses_width_driven_layout() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = diff_fixture("src/app.rs");
    let repo = gdl_core::open(fixture.path())?;

    let narrow_plain = diff_to_string(
        &repo,
        "src/app.rs",
        &plain_diff_options(60),
        DiffArea::Worktree,
    )?;
    let narrow_ansi = diff_to_string(
        &repo,
        "src/app.rs",
        &ansi_diff_options(60),
        DiffArea::Worktree,
    )?;
    let wide_plain = diff_to_string(
        &repo,
        "src/app.rs",
        &plain_diff_options(200),
        DiffArea::Worktree,
    )?;
    let wide_ansi = diff_to_string(
        &repo,
        "src/app.rs",
        &ansi_diff_options(200),
        DiffArea::Worktree,
    )?;

    assert_ne!(narrow_plain, wide_plain);
    assert!(narrow_plain.contains("--- a/src/app.rs"));
    assert!(wide_plain.contains(" | "));
    assert_eq!(
        strip_ansi_escapes::strip(narrow_ansi.as_bytes()),
        narrow_plain.as_bytes()
    );
    assert_eq!(
        strip_ansi_escapes::strip(wide_ansi.as_bytes()),
        wide_plain.as_bytes()
    );

    Ok(())
}

fn assert_token_color(output: &str, token: &str, expected_color: SetForegroundColor) {
    let token_index = output
        .find(token)
        .unwrap_or_else(|| panic!("missing token {token:?} in ANSI output:\n{output}"));
    assert_escape_before(output, token_index, expected_color, token);
}

fn assert_token_after_filename_color(
    output: &str,
    filename: &str,
    token: &str,
    expected_color: SetForegroundColor,
) {
    let filename_index = output
        .find(filename)
        .unwrap_or_else(|| panic!("missing filename {filename:?} in ANSI output:\n{output}"));
    let token_offset = output[filename_index..]
        .find(token)
        .unwrap_or_else(|| panic!("missing token {token:?} after {filename:?}"));
    assert_escape_before(output, filename_index + token_offset, expected_color, token);
}

fn assert_badge_color(
    output: &str,
    badge: char,
    filename: &str,
    expected_color: SetForegroundColor,
) {
    let filename_index = output
        .find(filename)
        .unwrap_or_else(|| panic!("missing filename {filename:?} in ANSI output:\n{output}"));
    let badge_index = output[..filename_index]
        .rfind(badge)
        .unwrap_or_else(|| panic!("missing badge {badge:?} before {filename:?}"));
    assert_escape_before(output, badge_index, expected_color, &badge.to_string());
}

fn assert_escape_before(
    output: &str,
    token_index: usize,
    expected_color: SetForegroundColor,
    label: &str,
) {
    let expected_escape = expected_color.to_string();
    let actual_start = token_index.saturating_sub(expected_escape.len());

    assert_eq!(
        &output[actual_start..token_index],
        expected_escape,
        "unexpected color before {label:?}"
    );
}

fn ansi_options() -> RenderOptions {
    RenderOptions {
        format: OutputFormat::Ansi,
        color: ColorPolicy::Always,
        width: 80,
        view: StatusView::Full,
    }
}

fn plain_options() -> RenderOptions {
    RenderOptions {
        format: OutputFormat::Plain,
        color: ColorPolicy::Never,
        width: 80,
        view: StatusView::Full,
    }
}

fn ansi_diff_options(width: usize) -> RenderOptions {
    RenderOptions {
        format: OutputFormat::Ansi,
        color: ColorPolicy::Always,
        width,
        view: StatusView::Full,
    }
}

fn plain_diff_options(width: usize) -> RenderOptions {
    RenderOptions {
        format: OutputFormat::Plain,
        color: ColorPolicy::Never,
        width,
        view: StatusView::Full,
    }
}

fn status_fixture<const N: usize>(mutation_order: [&str; N]) -> TestRepo {
    let fixture = TestRepo::init();
    fixture.write("nested/modified.txt", "base\n");
    fixture.write("deleted.txt", "delete me\n");
    fixture.write("old-name.txt", "rename me\n");
    fixture.git(["add", "."]);
    fixture.git(["commit", "-m", "initial"]);

    for path in mutation_order {
        match path {
            "nested/modified.txt" => fixture.write("nested/modified.txt", "base\nchanged\n"),
            "staged.txt" => {
                fixture.write("staged.txt", "staged\n");
                fixture.git(["add", "staged.txt"]);
            }
            "untracked.txt" => fixture.write("untracked.txt", "untracked\n"),
            other => panic!("unknown mutation path: {other}"),
        }
    }

    fixture.remove("deleted.txt");
    fixture.git(["mv", "old-name.txt", "renamed.txt"]);

    fixture
}

fn diff_fixture(path: &str) -> TestRepo {
    let fixture = TestRepo::init();
    fixture.write(path, "fn original() { let text = \"before\"; // old\n");
    fixture.git(["add", "."]);
    fixture.git(["commit", "-m", "initial"]);
    fixture.write(path, "fn changed() { let text = \"after\"; // new\n");

    fixture
}

fn syntect_oracle_line(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let syntax = syntax_set
        .find_syntax_for_file(path)?
        .expect("Rust syntax must be detected");
    let theme_set = ThemeSet::load_defaults();
    let theme = theme_set
        .themes
        .get("base16-ocean.dark")
        .expect("base16-ocean.dark theme must exist");
    let mut highlighter = HighlightLines::new(syntax, theme);
    let ranges =
        highlighter.highlight_line("fn changed() { let text = \"after\"; // new\n", &syntax_set)?;

    Ok(as_24_bit_terminal_escaped(&ranges, false))
}

fn conflicted_fixture() -> TestRepo {
    let fixture = TestRepo::init();
    fixture.write("conflict.txt", "base\n");
    fixture.git(["add", "."]);
    fixture.git(["commit", "-m", "base"]);

    fixture.git(["checkout", "-b", "theirs"]);
    fixture.write("conflict.txt", "theirs\n");
    fixture.git(["commit", "-am", "theirs"]);

    fixture.git(["checkout", "main"]);
    fixture.write("conflict.txt", "ours\n");
    fixture.git(["commit", "-am", "ours"]);

    let merge = fixture.try_git(["merge", "theirs"]);
    assert!(!merge.status.success(), "merge must create a conflict");

    fixture
}
