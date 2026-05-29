use gdl_core::{ChangeKind, ChangeSection, Repository};
use gdl_format::{
    status_to_string, ColorPolicy, OutputFormat, RenderOptions, StatusOutput, StatusView,
};
use gdl_testkit::TestRepo;
use serde_json::{json, Value};

#[test]
fn renders_full_status_as_json() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = status_fixture()?;
    let output = status_to_string(&fixture.repo, &json_options())?;

    let parsed: StatusOutput = serde_json::from_str(&output)?;
    let value: Value = serde_json::from_str(&output)?;
    let expected_entries = gdl_core::status(&fixture.repo)?;

    assert_eq!(parsed.version, gdl_core::version());
    assert_eq!(parsed.entries, expected_entries);
    assert_eq!(parsed.entries.len(), 3);
    assert_eq!(
        parsed
            .entries
            .iter()
            .map(|entry| entry.path.to_str().expect("test paths are UTF-8"))
            .collect::<Vec<_>>(),
        ["file.txt", "new.txt", "notes.txt"]
    );

    assert_eq!(
        value,
        json!({
            "version": gdl_core::version(),
            "entries": [
                {
                    "section": "Staged",
                    "kind": "Modified",
                    "path": "file.txt",
                    "old_path": null,
                    "lines_added": 1,
                    "lines_removed": 0,
                    "is_binary": false
                },
                {
                    "section": "Staged",
                    "kind": "Added",
                    "path": "new.txt",
                    "old_path": null,
                    "lines_added": 1,
                    "lines_removed": 0,
                    "is_binary": false
                },
                {
                    "section": "Untracked",
                    "kind": "Untracked",
                    "path": "notes.txt",
                    "old_path": null,
                    "lines_added": 1,
                    "lines_removed": 0,
                    "is_binary": false
                }
            ]
        })
    );

    assert_eq!(
        parsed
            .entries
            .iter()
            .map(|entry| (entry.section, entry.kind))
            .collect::<Vec<_>>(),
        vec![
            (ChangeSection::Staged, ChangeKind::Modified),
            (ChangeSection::Staged, ChangeKind::Added),
            (ChangeSection::Untracked, ChangeKind::Untracked),
        ]
    );

    Ok(())
}

fn status_fixture() -> Result<Fixture, Box<dyn std::error::Error>> {
    let temp_dir = TestRepo::init();

    temp_dir.write("file.txt", "old\n");
    temp_dir.git(["add", "."]);
    temp_dir.git(["commit", "-m", "initial"]);

    temp_dir.write("file.txt", "old\nstaged\n");
    temp_dir.git(["add", "file.txt"]);

    temp_dir.write("new.txt", "new\n");
    temp_dir.git(["add", "new.txt"]);

    temp_dir.write("notes.txt", "notes\n");

    let repo = gdl_core::open(temp_dir.path())?;
    Ok(Fixture {
        repo,
        _temp_dir: temp_dir,
    })
}

fn json_options() -> RenderOptions {
    RenderOptions {
        format: OutputFormat::Json,
        color: ColorPolicy::Never,
        view: StatusView::Full,
        width: 80,
    }
}

struct Fixture {
    repo: Repository,
    _temp_dir: TestRepo,
}
