use gdl_core::DiffArea;
use gdl_format::{diff_to_string, ColorPolicy, OutputFormat, RenderOptions, StatusView};
use gdl_testkit::TestRepo;

#[test]
fn status_plain_renders_sections_rows_and_counts() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = status_fixture(["nested/modified.txt", "staged.txt", "untracked.txt"]);
    let repo = gdl_core::open(fixture.path())?;

    let output = gdl_format::status_to_string(
        &repo,
        &RenderOptions {
            format: OutputFormat::Plain,
            color: ColorPolicy::Never,
            width: 80,
            view: StatusView::Full,
        },
    )?;

    assert_eq!(
        output,
        concat!(
            "Staged Changes (2)\n",
            "R  renamed.txt       .       +0 -0  from old-name.txt\n",
            "A  staged.txt        .       +1 -0\n",
            "\n",
            "Changes (2)\n",
            "D  deleted.txt       .       +0 -1\n",
            "M  modified.txt      nested  +1 -0\n",
            "\n",
            "Untracked (1)\n",
            "U  untracked.txt     .       +1 -0\n",
        )
    );

    Ok(())
}

#[test]
fn status_plain_output_is_sorted_independent_of_mutation_order(
) -> Result<(), Box<dyn std::error::Error>> {
    let first = status_fixture(["nested/modified.txt", "staged.txt", "untracked.txt"]);
    let second = status_fixture(["untracked.txt", "staged.txt", "nested/modified.txt"]);

    let first_repo = gdl_core::open(first.path())?;
    let second_repo = gdl_core::open(second.path())?;
    let options = RenderOptions {
        format: OutputFormat::Plain,
        color: ColorPolicy::Never,
        width: 80,
        view: StatusView::Full,
    };

    assert_eq!(
        gdl_format::status_to_string(&first_repo, &options)?,
        gdl_format::status_to_string(&second_repo, &options)?
    );

    Ok(())
}

#[test]
fn status_plain_renders_conflicted_section() -> Result<(), Box<dyn std::error::Error>> {
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

    let repo = gdl_core::open(fixture.path())?;
    let output = gdl_format::status_to_string(
        &repo,
        &RenderOptions {
            format: OutputFormat::Plain,
            color: ColorPolicy::Never,
            width: 80,
            view: StatusView::Full,
        },
    )?;

    assert_eq!(
        output,
        concat!("Merge Changes (1)\n", "!  conflict.txt      .       !\n",)
    );

    Ok(())
}

#[test]
fn diff_plain_renders_unified_text() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = diff_fixture();
    let repo = gdl_core::open(fixture.path())?;

    let output = diff_to_string(
        &repo,
        "src/app.rs",
        &plain_diff_options(80),
        DiffArea::Worktree,
    )?;

    assert_eq!(
        output,
        concat!(
            "diff --worktree src/app.rs\n",
            "--- a/src/app.rs\n",
            "+++ b/src/app.rs\n",
            "@@ -2 +2 @@\n",
            "-    let value = \"old\"; // before\n",
            "+    let value = \"new\"; // after\n",
        )
    );

    Ok(())
}

#[test]
fn diff_plain_renders_binary_marker() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = TestRepo::init();
    fixture.write("bin.dat", b"old\0bytes");
    fixture.git(["add", "."]);
    fixture.git(["commit", "-m", "initial"]);
    fixture.write("bin.dat", b"new\0bytes");

    let repo = gdl_core::open(fixture.path())?;
    let output = diff_to_string(
        &repo,
        "bin.dat",
        &plain_diff_options(80),
        DiffArea::Worktree,
    )?;

    assert_eq!(output, "Binary file bin.dat changed\n");

    Ok(())
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

fn diff_fixture() -> TestRepo {
    let fixture = TestRepo::init();
    fixture.write(
        "src/app.rs",
        concat!(
            "fn main() {\n",
            "    let value = \"old\"; // before\n",
            "    println!(\"{value}\");\n",
            "}\n",
        ),
    );
    fixture.git(["add", "."]);
    fixture.git(["commit", "-m", "initial"]);
    fixture.write(
        "src/app.rs",
        concat!(
            "fn main() {\n",
            "    let value = \"new\"; // after\n",
            "    println!(\"{value}\");\n",
            "}\n",
        ),
    );

    fixture
}

fn plain_diff_options(width: usize) -> RenderOptions {
    RenderOptions {
        format: OutputFormat::Plain,
        color: ColorPolicy::Never,
        width,
        view: StatusView::Full,
    }
}
