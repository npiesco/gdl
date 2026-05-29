use assert_cmd::Command;
use gdl_core::DiffArea;
use gdl_format::{ColorPolicy, OutputFormat, RenderOptions, StatusView};
use gdl_testkit::TestRepo;

#[test]
fn diff_worktree_matches_formatter_bytes() -> Result<(), Box<dyn std::error::Error>> {
    assert_diff_matches(DiffArea::Worktree, false)
}

#[test]
fn diff_staged_matches_formatter_bytes() -> Result<(), Box<dyn std::error::Error>> {
    assert_diff_matches(DiffArea::Staged, true)
}

#[test]
fn diff_head_matches_formatter_bytes() -> Result<(), Box<dyn std::error::Error>> {
    assert_diff_matches(DiffArea::Head, false)
}

fn assert_diff_matches(area: DiffArea, staged: bool) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = diff_fixture(staged);
    let repo = gdl_core::open(fixture.path())?;
    let options = RenderOptions {
        format: OutputFormat::Ansi,
        color: ColorPolicy::Always,
        width: 200,
        view: StatusView::Full,
    };
    let expected = gdl_format::diff_to_string(&repo, "file.txt", &options, area)?;

    let output = Command::cargo_bin("gdl")?
        .arg("--repo")
        .arg(fixture.path())
        .args([
            "--color", "always", "--width", "200", "diff", "file.txt", "--area",
        ])
        .arg(area.to_string())
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

fn diff_fixture(staged: bool) -> TestRepo {
    let fixture = TestRepo::init();
    fixture.write("file.txt", "one\nold\nthree\n");
    fixture.git(["add", "."]);
    fixture.git(["commit", "-m", "initial"]);
    fixture.write("file.txt", "one\nnew\nthree\n");
    if staged {
        fixture.git(["add", "file.txt"]);
    }

    fixture
}
