use std::path::PathBuf;

use gdl_core::{ChangeKind, ChangeSection, GdlEntry};
use gdl_testkit::TestRepo;

#[test]
fn status_reports_conflicted_paths_once() -> Result<(), Box<dyn std::error::Error>> {
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
    let entries = gdl_core::status(&repo)?;

    assert_eq!(
        entries,
        vec![GdlEntry {
            section: ChangeSection::Conflicted,
            kind: ChangeKind::Conflicted,
            path: PathBuf::from("conflict.txt"),
            old_path: None,
            lines_added: 0,
            lines_removed: 0,
            is_binary: false,
        }]
    );

    Ok(())
}
