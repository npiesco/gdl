use gdl_core::{ChangeKind, ChangeSection, DiffArea, Hunk, Repository};
use gdl_format::{
    diff_to_string, status_to_string, ColorPolicy, DiffOutput, OutputFormat, RenderOptions,
    StatusOutput, StatusView,
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

#[test]
fn renders_diff_as_json() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = diff_fixture(false);
    let repo = gdl_core::open(fixture.path())?;

    let output = diff_to_string(&repo, "file.txt", &json_options(), DiffArea::Worktree)?;
    let parsed: DiffOutput = serde_json::from_str(&output)?;
    let value: Value = serde_json::from_str(&output)?;

    assert_eq!(
        parsed,
        DiffOutput {
            version: gdl_core::version().to_owned(),
            file: "file.txt".to_owned(),
            area: DiffArea::Worktree,
            width: 80,
            binary: false,
            hunks: vec![Hunk {
                old_start: 2,
                old_lines: 1,
                new_start: 2,
                new_lines: 1,
                old_bytes: vec![b"old\n".to_vec()],
                new_bytes: vec![b"new\n".to_vec()],
            }],
        }
    );
    assert_eq!(
        value,
        json!({
            "version": gdl_core::version(),
            "file": "file.txt",
            "area": "worktree",
            "width": 80,
            "binary": false,
            "hunks": [
                {
                    "old_start": 2,
                    "old_lines": 1,
                    "new_start": 2,
                    "new_lines": 1,
                    "old_bytes": [[111, 108, 100, 10]],
                    "new_bytes": [[110, 101, 119, 10]]
                }
            ]
        })
    );

    Ok(())
}

#[test]
fn renders_binary_diff_as_json() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = diff_fixture(true);
    let repo = gdl_core::open(fixture.path())?;

    let output = diff_to_string(&repo, "file.txt", &json_options(), DiffArea::Worktree)?;
    let parsed: DiffOutput = serde_json::from_str(&output)?;

    assert!(parsed.binary);
    assert_eq!(parsed.hunks, Vec::<Hunk>::new());

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

fn diff_fixture(binary: bool) -> TestRepo {
    let fixture = TestRepo::init();
    if binary {
        fixture.write("file.txt", b"old\0bytes");
        fixture.git(["add", "."]);
        fixture.git(["commit", "-m", "initial"]);
        fixture.write("file.txt", b"new\0bytes");
    } else {
        fixture.write("file.txt", "one\nold\nthree\n");
        fixture.git(["add", "."]);
        fixture.git(["commit", "-m", "initial"]);
        fixture.write("file.txt", "one\nnew\nthree\n");
    }

    fixture
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
