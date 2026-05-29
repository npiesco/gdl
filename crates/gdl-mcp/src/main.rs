//! gdl-mcp stdio server.

use std::path::PathBuf;

use gdl_core::DiffArea;
use gdl_format::{ColorPolicy, OutputFormat, RenderOptions, StatusView};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
    ErrorData as McpError, ServerHandler, ServiceExt,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
struct GdlMcpServer {
    tool_router: ToolRouter<Self>,
    server_info: Implementation,
}

impl GdlMcpServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            server_info: Implementation::new("gdl-mcp", gdl_core::version()),
        }
    }

    async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let service = self.serve(stdio()).await?;
        service.waiting().await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
enum McpOutputFormat {
    #[default]
    Plain,
    Ansi,
    Json,
}

impl From<McpOutputFormat> for OutputFormat {
    fn from(value: McpOutputFormat) -> Self {
        match value {
            McpOutputFormat::Plain => OutputFormat::Plain,
            McpOutputFormat::Ansi => OutputFormat::Ansi,
            McpOutputFormat::Json => OutputFormat::Json,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
enum McpColorPolicy {
    #[default]
    Never,
    Always,
}

impl From<McpColorPolicy> for ColorPolicy {
    fn from(value: McpColorPolicy) -> Self {
        match value {
            McpColorPolicy::Never => ColorPolicy::Never,
            McpColorPolicy::Always => ColorPolicy::Always,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum McpStatusView {
    #[default]
    Full,
    PathsOnly,
}

impl From<McpStatusView> for StatusView {
    fn from(value: McpStatusView) -> Self {
        match value {
            McpStatusView::Full => StatusView::Full,
            McpStatusView::PathsOnly => StatusView::PathsOnly,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
enum McpDiffArea {
    #[default]
    Worktree,
    Staged,
    Head,
}

impl From<McpDiffArea> for DiffArea {
    fn from(value: McpDiffArea) -> Self {
        match value {
            McpDiffArea::Worktree => DiffArea::Worktree,
            McpDiffArea::Staged => DiffArea::Staged,
            McpDiffArea::Head => DiffArea::Head,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct StatusParams {
    /// Repository path to open or discover upward from.
    repo: PathBuf,
    /// Output format: plain, ansi, or json.
    #[serde(default)]
    format: McpOutputFormat,
    /// ANSI color policy: never or always.
    #[serde(default)]
    color: McpColorPolicy,
    /// Deterministic render width.
    #[serde(default = "default_width")]
    width: usize,
    /// Status view: full or paths-only.
    #[serde(default)]
    view: McpStatusView,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct DiffParams {
    /// Repository path to open or discover upward from.
    repo: PathBuf,
    /// Repository-relative file path to diff.
    path: PathBuf,
    /// Output format: plain, ansi, or json.
    #[serde(default)]
    format: McpOutputFormat,
    /// ANSI color policy: never or always.
    #[serde(default)]
    color: McpColorPolicy,
    /// Deterministic render width.
    #[serde(default = "default_width")]
    width: usize,
    /// Status view field kept for CLI/MCP RenderOptions parity.
    #[serde(default)]
    view: McpStatusView,
    /// Diff area: worktree, staged, or head.
    #[serde(default)]
    area: McpDiffArea,
}

fn default_width() -> usize {
    80
}

impl StatusParams {
    fn render_options(&self) -> RenderOptions {
        RenderOptions {
            format: self.format.into(),
            color: self.color.into(),
            width: self.width,
            view: self.view.into(),
        }
    }
}

impl DiffParams {
    fn render_options(&self) -> RenderOptions {
        RenderOptions {
            format: self.format.into(),
            color: self.color.into(),
            width: self.width,
            view: self.view.into(),
        }
    }
}

fn tool_success(content: String) -> CallToolResult {
    CallToolResult::success(vec![Content::text(content)])
}

fn tool_error(message: String) -> CallToolResult {
    CallToolResult::error(vec![Content::text(message)])
}

#[tool_router]
impl GdlMcpServer {
    #[tool(description = "Render a gdl diff for a repository-relative file path.")]
    async fn diff(
        &self,
        Parameters(params): Parameters<DiffParams>,
    ) -> Result<CallToolResult, McpError> {
        let repo = match gdl_core::open(&params.repo) {
            Ok(repo) => repo,
            Err(err) => return Ok(tool_error(err.to_string())),
        };
        match gdl_format::diff_to_string(
            &repo,
            &params.path,
            &params.render_options(),
            params.area.into(),
        ) {
            Ok(output) => Ok(tool_success(output)),
            Err(err) => Ok(tool_error(err)),
        }
    }

    #[tool(description = "Render gdl status for a git repository.")]
    async fn status(
        &self,
        Parameters(params): Parameters<StatusParams>,
    ) -> Result<CallToolResult, McpError> {
        let repo = match gdl_core::open(&params.repo) {
            Ok(repo) => repo,
            Err(err) => return Ok(tool_error(err.to_string())),
        };
        match gdl_format::status_to_string(&repo, &params.render_options()) {
            Ok(output) => Ok(tool_success(output)),
            Err(err) => Ok(tool_error(err)),
        }
    }

    #[tool(description = "Return the gdl-core package version.")]
    async fn version(&self) -> Result<CallToolResult, McpError> {
        Ok(tool_success(gdl_core::version().to_owned()))
    }
}

impl Default for GdlMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for GdlMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
            .with_server_info(self.server_info.clone())
            .with_instructions(
                "Use status, diff, and version for byte-identical gdl CLI/MCP rendering.",
            )
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    GdlMcpServer::new().run().await
}
