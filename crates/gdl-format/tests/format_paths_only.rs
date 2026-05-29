use gdl_format::{status_to_string, ColorPolicy, OutputFormat, RenderOptions, StatusView};
use gdl_testkit::TestRepo;
use serde_json::{json, Value};

#[test]
fn status_paths_only_plain_renders_ordered_paths() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = status_fixture();
    let repo = gdl_core::open(fixture.path())?;

    let output = status_to_string(
        &repo,
        &RenderOptions {
            format: OutputFormat::Plain,
            color: ColorPolicy::Never,
            width: 80,
            view: StatusView::PathsOnly,
        },
    )?;

    assert_eq!(
        output,
        concat!(
            "renamed.txt\n",
            "staged.txt\n",
            "deleted.txt\n",
            "nested/modified.txt\n",
            "untracked.txt\n",
        )
    );

    Ok(())
}

#[test]
fn status_paths_only_json_renders_path_array() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = status_fixture();
    let repo = gdl_core::open(fixture.path())?;

    let output = status_to_string(
        &repo,
        &RenderOptions {
            format: OutputFormat::Json,
            color: ColorPolicy::Never,
            width: 80,
            view: StatusView::PathsOnly,
        },
    )?;

    let paths: Vec<String> = serde_json::from_str(&output)?;
    let value: Value = serde_json::from_str(&output)?;

    assert_eq!(
        paths,
        [
            "renamed.txt",
            "staged.txt",
            "deleted.txt",
            "nested/modified.txt",
            "untracked.txt",
        ]
    );
    assert_eq!(
        value,
        json!([
            "renamed.txt",
            "staged.txt",
            "deleted.txt",
            "nested/modified.txt",
            "untracked.txt",
        ])
    );

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
