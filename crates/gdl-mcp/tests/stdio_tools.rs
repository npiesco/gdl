use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use gdl_core::DiffArea;
use gdl_format::{
    diff_to_string, status_to_string, ColorPolicy, OutputFormat, RenderOptions, StatusView,
};
use gdl_testkit::TestRepo;
use serde_json::{json, Value};

struct McpProcess {
    child: Child,
    stdin: ChildStdin,
    stdout_rx: Receiver<String>,
}

impl McpProcess {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_gdl-mcp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("gdl-mcp must start");

        let stdout = child.stdout.take().expect("stdout must be piped");
        let stdin = child.stdin.take().expect("stdin must be piped");
        let (stdout_tx, stdout_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        stdout_tx.send(line).ok();
                    }
                }
            }
        });

        let mut process = Self {
            child,
            stdin,
            stdout_rx,
        };
        process.initialize();
        process
    }

    fn initialize(&mut self) {
        self.send(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "gdl-mcp-integration-test",
                    "version": "0.0.0"
                }
            }
        }));
        let response = self.response(1);
        assert_eq!(response["result"]["serverInfo"]["name"], "gdl-mcp");

        self.send(json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        }));
    }

    fn send(&mut self, message: Value) {
        serde_json::to_writer(&mut self.stdin, &message).expect("message must serialize");
        self.stdin.write_all(b"\n").expect("message newline");
        self.stdin.flush().expect("message flush");
    }

    fn request(&mut self, id: u64, method: &str, params: Value) -> Value {
        self.send(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }));
        self.response(id)
    }

    fn response(&self, id: u64) -> Value {
        for _ in 0..20 {
            let line = self
                .stdout_rx
                .recv_timeout(Duration::from_millis(250))
                .expect("gdl-mcp must respond");
            let value: Value = serde_json::from_str(&line).expect("stdout must be JSON-RPC");
            if value.get("id").and_then(Value::as_u64) == Some(id) {
                return value;
            }
        }

        panic!("gdl-mcp did not return response id {id}");
    }
}

impl Drop for McpProcess {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}

fn text_result(response: Value) -> String {
    assert_eq!(response["result"]["isError"], false);
    response["result"]["content"][0]["text"]
        .as_str()
        .expect("tool result must contain text")
        .to_owned()
}

fn error_text(response: Value) -> String {
    assert_eq!(response["result"]["isError"], true);
    response["result"]["content"][0]["text"]
        .as_str()
        .expect("tool error must contain text")
        .to_owned()
}

fn fixture() -> TestRepo {
    let repo = TestRepo::init();
    repo.write("tracked.txt", "before\n");
    repo.git(["add", "."]);
    repo.git(["commit", "-m", "initial"]);
    repo.write("tracked.txt", "before\nafter\n");
    repo.write("staged.txt", "staged\n");
    repo.git(["add", "staged.txt"]);
    repo
}

#[test]
fn stdio_lists_gdl_tools() {
    let mut mcp = McpProcess::start();

    let response = mcp.request(2, "tools/list", json!({}));
    let tools = response["result"]["tools"]
        .as_array()
        .expect("tools/list must return tools");
    let names = tools
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name"))
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["diff", "status", "version"]);
}

#[test]
fn status_tool_matches_formatter_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture();
    let repo = gdl_core::open(fixture.path())?;
    let options = RenderOptions {
        format: OutputFormat::Plain,
        color: ColorPolicy::Never,
        width: 80,
        view: StatusView::Full,
    };
    let expected = status_to_string(&repo, &options)?;

    let mut mcp = McpProcess::start();
    let response = mcp.request(
        2,
        "tools/call",
        json!({
            "name": "status",
            "arguments": {
                "repo": fixture.path(),
                "format": "plain",
                "color": "never",
                "width": 80,
                "view": "full"
            }
        }),
    );

    assert_eq!(text_result(response), expected);
    Ok(())
}

#[test]
fn diff_tool_matches_formatter_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture();
    let repo = gdl_core::open(fixture.path())?;
    let options = RenderOptions {
        format: OutputFormat::Plain,
        color: ColorPolicy::Never,
        width: 80,
        view: StatusView::Full,
    };
    let expected = diff_to_string(&repo, "tracked.txt", &options, DiffArea::Worktree)?;

    let mut mcp = McpProcess::start();
    let response = mcp.request(
        2,
        "tools/call",
        json!({
            "name": "diff",
            "arguments": {
                "repo": fixture.path(),
                "path": "tracked.txt",
                "format": "plain",
                "color": "never",
                "width": 80,
                "view": "full",
                "area": "worktree"
            }
        }),
    );

    assert_eq!(text_result(response), expected);
    Ok(())
}

#[test]
fn version_tool_returns_core_version() {
    let mut mcp = McpProcess::start();
    let response = mcp.request(
        2,
        "tools/call",
        json!({
            "name": "version",
            "arguments": {}
        }),
    );

    assert_eq!(text_result(response), gdl_core::version());
}

#[test]
fn status_tool_reports_open_errors_as_tool_errors() {
    let fixture = TestRepo::init();
    let missing = fixture.path().join("missing");

    let mut mcp = McpProcess::start();
    let response = mcp.request(
        2,
        "tools/call",
        json!({
            "name": "status",
            "arguments": {
                "repo": missing,
                "format": "plain",
                "color": "never",
                "width": 80,
                "view": "full"
            }
        }),
    );

    assert_eq!(
        error_text(response),
        format!("path does not exist: {}", missing.display())
    );
}
