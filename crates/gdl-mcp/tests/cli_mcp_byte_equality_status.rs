mod common;

use common::{git_status_snapshot, run_gdl, tool_text, workspace_root, McpProcess};
use gdl_testkit::TestRepo;
use serde_json::json;

#[derive(Debug, Clone, Copy)]
struct StatusCase {
    format: &'static str,
    color: &'static str,
    width: usize,
    view: &'static str,
    cli_args: &'static [&'static str],
}

const STATUS_CASES: &[StatusCase] = &[
    StatusCase {
        format: "plain",
        color: "never",
        width: 80,
        view: "full",
        cli_args: &[
            "--format", "plain", "--color", "never", "--width", "80", "status",
        ],
    },
    StatusCase {
        format: "ansi",
        color: "always",
        width: 120,
        view: "full",
        cli_args: &[
            "--format", "ansi", "--color", "always", "--width", "120", "status",
        ],
    },
    StatusCase {
        format: "json",
        color: "never",
        width: 80,
        view: "full",
        cli_args: &[
            "--format", "json", "--color", "never", "--width", "80", "status",
        ],
    },
    StatusCase {
        format: "plain",
        color: "never",
        width: 80,
        view: "paths-only",
        cli_args: &[
            "--format",
            "plain",
            "--color",
            "never",
            "--paths-only",
            "--width",
            "80",
            "status",
        ],
    },
];

#[test]
fn fixture_status_cli_and_mcp_are_byte_equal_for_render_matrix() {
    let fixture = status_fixture();

    for (index, case) in STATUS_CASES.iter().enumerate() {
        let cli = run_gdl(fixture.path(), case.cli_args);
        let mut mcp = McpProcess::start();
        let mcp = tool_text(mcp.call_tool(
            (index + 2) as u64,
            "status",
            json!({
                "repo": fixture.path().display().to_string(),
                "format": case.format,
                "color": case.color,
                "width": case.width,
                "view": case.view
            }),
        ));

        assert_eq!(cli, mcp, "status CLI/MCP mismatch for {case:?}");
    }
}

#[test]
fn dogfood_status_cli_and_mcp_are_byte_equal_with_race_guard() {
    let repo = workspace_root();

    for (index, case) in STATUS_CASES.iter().enumerate() {
        let before = git_status_snapshot(repo);
        let cli = run_gdl(repo, case.cli_args);
        let mut mcp = McpProcess::start();
        let mcp = tool_text(mcp.call_tool(
            (index + 2) as u64,
            "status",
            json!({
                "repo": repo.display().to_string(),
                "format": case.format,
                "color": case.color,
                "width": case.width,
                "view": case.view
            }),
        ));
        let after = git_status_snapshot(repo);

        assert_eq!(before, after, "repo changed during dogfood test — rerun");
        assert_eq!(cli, mcp, "dogfood status CLI/MCP mismatch for {case:?}");
    }
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
