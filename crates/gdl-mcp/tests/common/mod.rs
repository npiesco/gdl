use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::Duration;

use serde_json::{json, Value};

pub struct McpProcess {
    child: Child,
    stdin: ChildStdin,
    stdout_rx: Receiver<String>,
}

impl McpProcess {
    pub fn start() -> Self {
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

    pub fn call_tool(&mut self, id: u64, name: &str, arguments: Value) -> Value {
        self.request(
            id,
            "tools/call",
            json!({
                "name": name,
                "arguments": arguments
            }),
        )
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
                    "name": "gdl-mcp-byte-equality-test",
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
            let line = match self.stdout_rx.recv_timeout(Duration::from_millis(250)) {
                Ok(line) => line,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => panic!("gdl-mcp stdout disconnected"),
            };
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

pub fn tool_text(response: Value) -> Vec<u8> {
    assert_eq!(response["result"]["isError"], false);
    response["result"]["content"][0]["text"]
        .as_str()
        .expect("tool result must contain text")
        .as_bytes()
        .to_vec()
}

pub fn run_gdl(repo: &Path, args: &[&str]) -> Vec<u8> {
    let output = Command::new(assert_cmd::cargo::cargo_bin("gdl"))
        .arg("--repo")
        .arg(repo)
        .args(args)
        .output()
        .expect("gdl must run");

    assert!(
        output.status.success(),
        "gdl failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stderr, b"");

    output.stdout
}

pub fn git_status_snapshot(repo: &Path) -> Vec<u8> {
    let output = Command::new("git")
        .args(["status", "--porcelain=v2", "-z"])
        .current_dir(repo)
        .output()
        .expect("git status must run");

    assert!(
        output.status.success(),
        "git status failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    output.stdout
}

pub fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("gdl-mcp crate must live under workspace/crates/gdl-mcp")
}
