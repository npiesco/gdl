use gdl_core::{DiffArea, Hunk};
use gdl_testkit::TestRepo;

#[test]
fn worktree_diff_returns_real_hunks() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = diff_fixture(false);
    let repo = gdl_core::open(fixture.path())?;

    let diff = gdl_core::diff(&repo, "file.txt", DiffArea::Worktree)?;

    assert_eq!(diff.file, std::path::PathBuf::from("file.txt"));
    assert_eq!(diff.area, DiffArea::Worktree);
    assert!(!diff.binary);
    assert_eq!(diff.hunks, expected_hunks());

    Ok(())
}

#[test]
fn staged_diff_returns_real_hunks() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = diff_fixture(true);
    let repo = gdl_core::open(fixture.path())?;

    let diff = gdl_core::diff(&repo, "file.txt", DiffArea::Staged)?;

    assert_eq!(diff.area, DiffArea::Staged);
    assert_eq!(diff.hunks, expected_hunks());

    Ok(())
}

#[test]
fn head_diff_returns_real_hunks() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = diff_fixture(false);
    let repo = gdl_core::open(fixture.path())?;

    let diff = gdl_core::diff(&repo, "file.txt", DiffArea::Head)?;

    assert_eq!(diff.area, DiffArea::Head);
    assert_eq!(diff.hunks, expected_hunks());

    Ok(())
}

#[test]
fn diff_preserves_crlf_and_non_utf8_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = TestRepo::init();
    fixture.write("raw.txt", b"one\r\nold \xff\r\nthree\r\n");
    fixture.git(["add", "."]);
    fixture.git(["commit", "-m", "initial"]);
    fixture.write("raw.txt", b"one\r\nnew \xff\r\nthree\r\n");

    let repo = gdl_core::open(fixture.path())?;
    let diff = gdl_core::diff(&repo, "raw.txt", DiffArea::Worktree)?;

    assert_eq!(
        diff.hunks,
        vec![Hunk {
            old_start: 2,
            old_lines: 1,
            new_start: 2,
            new_lines: 1,
            old_bytes: vec![b"old \xff\r\n".to_vec()],
            new_bytes: vec![b"new \xff\r\n".to_vec()],
        }]
    );

    Ok(())
}

fn diff_fixture(staged: bool) -> TestRepo {
    let fixture = TestRepo::init();
    fixture.write("file.txt", old_bytes());
    fixture.git(["add", "."]);
    fixture.git(["commit", "-m", "initial"]);
    fixture.write("file.txt", new_bytes());
    if staged {
        fixture.git(["add", "file.txt"]);
    }

    fixture
}

fn old_bytes() -> &'static [u8] {
    b"one\nold two\nthree\nfour\nold five\nsix\nseven\nold eight\nnine\n"
}

fn new_bytes() -> &'static [u8] {
    b"one\nnew two\nthree\nfour\nnew five\nsix\nseven\nnew eight\nnine\n"
}

fn expected_hunks() -> Vec<Hunk> {
    vec![
        Hunk {
            old_start: 2,
            old_lines: 1,
            new_start: 2,
            new_lines: 1,
            old_bytes: vec![b"old two\n".to_vec()],
            new_bytes: vec![b"new two\n".to_vec()],
        },
        Hunk {
            old_start: 5,
            old_lines: 1,
            new_start: 5,
            new_lines: 1,
            old_bytes: vec![b"old five\n".to_vec()],
            new_bytes: vec![b"new five\n".to_vec()],
        },
        Hunk {
            old_start: 8,
            old_lines: 1,
            new_start: 8,
            new_lines: 1,
            old_bytes: vec![b"old eight\n".to_vec()],
            new_bytes: vec![b"new eight\n".to_vec()],
        },
    ]
}
