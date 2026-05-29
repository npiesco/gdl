use assert_cmd::Command;
use gdl_format::{ColorPolicy, OutputFormat, RenderOptions, StatusView};
use gdl_testkit::TestRepo;

#[test]
fn status_color_always_matches_formatter_bytes() -> Result<(), Box<dyn std::error::Error>> {
    assert_status_matches(
        ["--color", "always", "--width", "120", "status"],
        RenderOptions {
            format: OutputFormat::Ansi,
            color: ColorPolicy::Always,
            width: 120,
            view: StatusView::Full,
        },
    )
}

#[test]
fn status_color_never_matches_formatter_bytes() -> Result<(), Box<dyn std::error::Error>> {
    assert_status_matches(
        ["--color", "never", "--width", "120", "status"],
        RenderOptions {
            format: OutputFormat::Plain,
            color: ColorPolicy::Never,
            width: 120,
            view: StatusView::Full,
        },
    )
}

#[test]
fn status_paths_only_matches_formatter_bytes() -> Result<(), Box<dyn std::error::Error>> {
    assert_status_matches(
        [
            "--color",
            "never",
            "--paths-only",
            "--width",
            "80",
            "status",
        ],
        RenderOptions {
            format: OutputFormat::Plain,
            color: ColorPolicy::Never,
            width: 80,
            view: StatusView::PathsOnly,
        },
    )
}

#[test]
fn status_json_matches_formatter_bytes() -> Result<(), Box<dyn std::error::Error>> {
    assert_status_matches(
        [
            "--color", "never", "--format", "json", "--width", "80", "status",
        ],
        RenderOptions {
            format: OutputFormat::Json,
            color: ColorPolicy::Never,
            width: 80,
            view: StatusView::Full,
        },
    )
}

fn assert_status_matches<const N: usize>(
    args: [&str; N],
    options: RenderOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = status_fixture();
    let repo = gdl_core::open(fixture.path())?;
    let expected = gdl_format::status_to_string(&repo, &options)?;

    let output = Command::cargo_bin("gdl")?
        .arg("--repo")
        .arg(fixture.path())
        .args(args)
        .output()?;

    assert!(
        output.status.success(),
        "gdl failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stderr, b"");
    assert_eq!(output.stdout, expected.as_bytes());

    Ok(())
}

fn status_fixture() -> TestRepo {
    let fixture = TestRepo::init();
    fixture.write("nested/modified.txt", "base\n");
    fixture.write("deleted.txt", "delete me\n");
    fixture.write("old-name.txt", "rename me\n");
    fixture.git(["add", "."]);
    fixture.git(["commit", "-m", "initial"]);

    fixture.write("nested/modified.txt", "base\nchanged\n");
    fixture.write("staged.txt", "staged\n");
    fixture.git(["add", "staged.txt"]);
    fixture.write("untracked.txt", "untracked\n");
    fixture.remove("deleted.txt");
    fixture.git(["mv", "old-name.txt", "renamed.txt"]);

    fixture
}
