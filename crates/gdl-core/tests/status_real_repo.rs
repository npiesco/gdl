use std::path::PathBuf;

use gdl_core::{ChangeKind, ChangeSection, GdlEntry};
use gdl_testkit::TestRepo;

fn entry(
    section: ChangeSection,
    kind: ChangeKind,
    path: &str,
    old_path: Option<&str>,
    lines_added: usize,
    lines_removed: usize,
    is_binary: bool,
) -> GdlEntry {
    GdlEntry {
        section,
        kind,
        path: PathBuf::from(path),
        old_path: old_path.map(PathBuf::from),
        lines_added,
        lines_removed,
        is_binary,
    }
}

#[test]
fn status_reports_sections_kinds_counts_and_stable_order() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = TestRepo::init();
    fixture.write("keep.txt", "hello\n");
    fixture.write("delete-me.txt", "delete me\n");
    fixture.write("old-name.txt", "rename me\n");
    fixture.git(["add", "."]);
    fixture.git(["commit", "-m", "initial"]);

    fixture.write("keep.txt", "hello\nchanged\n");
    fixture.write("untracked.txt", "untracked\n");
    fixture.write("added.txt", "added\n");
    fixture.git(["add", "added.txt"]);
    fixture.remove("delete-me.txt");
    fixture.git(["mv", "old-name.txt", "new-name.txt"]);

    let repo = gdl_core::open(fixture.path())?;
    let entries = gdl_core::status(&repo)?;

    assert_eq!(
        entries,
        vec![
            entry(
                ChangeSection::Staged,
                ChangeKind::Added,
                "added.txt",
                None,
                1,
                0,
                false,
            ),
            entry(
                ChangeSection::Staged,
                ChangeKind::Renamed,
                "new-name.txt",
                Some("old-name.txt"),
                0,
                0,
                false,
            ),
            entry(
                ChangeSection::WorkingTree,
                ChangeKind::Deleted,
                "delete-me.txt",
                None,
                0,
                1,
                false,
            ),
            entry(
                ChangeSection::WorkingTree,
                ChangeKind::Modified,
                "keep.txt",
                None,
                1,
                0,
                false,
            ),
            entry(
                ChangeSection::Untracked,
                ChangeKind::Untracked,
                "untracked.txt",
                None,
                1,
                0,
                false,
            ),
        ]
    );

    Ok(())
}
