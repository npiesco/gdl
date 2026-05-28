use std::path::PathBuf;

use gdl_core::{ChangeKind, ChangeSection, GdlEntry};
use gdl_testkit::TestRepo;

#[test]
fn status_marks_binary_changes_without_line_counts() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = TestRepo::init();
    fixture.write("image.bin", [0, 159, 146, 150, 0]);
    fixture.git(["add", "."]);
    fixture.git(["commit", "-m", "binary"]);

    fixture.write("image.bin", [0, 159, 146, 151, 0]);

    let repo = gdl_core::open(fixture.path())?;
    let entries = gdl_core::status(&repo)?;

    assert_eq!(
        entries,
        vec![GdlEntry {
            section: ChangeSection::WorkingTree,
            kind: ChangeKind::Modified,
            path: PathBuf::from("image.bin"),
            old_path: None,
            lines_added: 0,
            lines_removed: 0,
            is_binary: true,
        }]
    );

    Ok(())
}
