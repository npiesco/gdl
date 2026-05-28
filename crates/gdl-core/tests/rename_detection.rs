use std::path::PathBuf;

use gdl_core::{ChangeKind, ChangeSection, GdlEntry};
use gdl_testkit::TestRepo;

#[test]
fn status_reports_staged_rename_with_old_path() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = TestRepo::init();
    fixture.write("old.txt", "same bytes\n");
    fixture.git(["add", "."]);
    fixture.git(["commit", "-m", "initial"]);

    fixture.git(["mv", "old.txt", "new.txt"]);

    let repo = gdl_core::open(fixture.path())?;
    let entries = gdl_core::status(&repo)?;

    assert_eq!(
        entries,
        vec![GdlEntry {
            section: ChangeSection::Staged,
            kind: ChangeKind::Renamed,
            path: PathBuf::from("new.txt"),
            old_path: Some(PathBuf::from("old.txt")),
            lines_added: 0,
            lines_removed: 0,
            is_binary: false,
        }]
    );

    Ok(())
}
