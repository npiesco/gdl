mod common;

use common::{git_status_snapshot, run_gdl, tool_text, workspace_root, McpProcess};
use gdl_testkit::TestRepo;
use serde_json::json;

#[derive(Debug, Clone, Copy)]
struct DiffCase {
    format: &'static str,
    color: &'static str,
    width: usize,
    area: &'static str,
    cli_prefix: &'static [&'static str],
}

const DIFF_CASES: &[DiffCase] = &[
    DiffCase {
        format: "plain",
        color: "never",
        width: 80,
        area: "worktree",
        cli_prefix: &["--format", "plain", "--color", "never", "--width", "80"],
    },
    DiffCase {
        format: "ansi",
        color: "always",
        width: 120,
        area: "staged",
        cli_prefix: &["--format", "ansi", "--color", "always", "--width", "120"],
    },
    DiffCase {
        format: "json",
        color: "never",
        width: 80,
        area: "head",
        cli_prefix: &["--format", "json", "--color", "never", "--width", "80"],
    },
];

#[test]
fn fixture_diff_cli_and_mcp_are_byte_equal_for_render_matrix() {
    let fixture = diff_fixture();

    for (index, case) in DIFF_CASES.iter().enumerate() {
        let cli = run_gdl(fixture.path(), &diff_cli_args(case, "file.txt"));
        let mut mcp = McpProcess::start();
        let mcp = tool_text(mcp.call_tool(
            (index + 2) as u64,
            "diff",
            json!({
                "repo": fixture.path().display().to_string(),
                "path": "file.txt",
                "format": case.format,
                "color": case.color,
                "width": case.width,
                "view": "full",
                "area": case.area
            }),
        ));

        assert_eq!(cli, mcp, "diff CLI/MCP mismatch for {case:?}");
    }
}

#[test]
fn dogfood_diff_cli_and_mcp_are_byte_equal_with_race_guard() {
    let repo = workspace_root();

    for (index, case) in DIFF_CASES.iter().enumerate() {
        let before = git_status_snapshot(repo);
        let cli = run_gdl(repo, &diff_cli_args(case, "PLAN.md"));
        let mut mcp = McpProcess::start();
        let mcp = tool_text(mcp.call_tool(
            (index + 2) as u64,
            "diff",
            json!({
                "repo": repo.display().to_string(),
                "path": "PLAN.md",
                "format": case.format,
                "color": case.color,
                "width": case.width,
                "view": "full",
                "area": case.area
            }),
        ));
        let after = git_status_snapshot(repo);

        assert_eq!(before, after, "repo changed during dogfood test — rerun");
        assert_eq!(cli, mcp, "dogfood diff CLI/MCP mismatch for {case:?}");
    }
}

fn diff_fixture() -> TestRepo {
    let fixture = TestRepo::init();
    fixture.write("file.txt", "one\nbase\nthree\n");
    fixture.git(["add", "."]);
    fixture.git(["commit", "-m", "initial"]);

    fixture.write("file.txt", "one\nstaged\nthree\n");
    fixture.git(["add", "file.txt"]);
    fixture.write("file.txt", "one\nworktree\nthree\n");

    fixture
}

fn diff_cli_args(case: &DiffCase, path: &'static str) -> Vec<&'static str> {
    let mut args = case.cli_prefix.to_vec();
    args.extend(["diff", path, "--area", case.area]);
    args
}
