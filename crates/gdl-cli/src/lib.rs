//! gdl CLI facade over gdl-core and gdl-format.

pub mod options;

use std::io::{self, Write};
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// gdl command-line arguments.
#[derive(Debug, Parser)]
#[command(name = "gdl", version, about = "Git diff and status helpers")]
pub struct Cli {
    /// Repository path to inspect.
    #[arg(long, value_name = "PATH", global = true)]
    pub repo: Option<PathBuf>,

    /// ANSI color policy.
    #[arg(long, value_enum, default_value_t = ColorArg::Auto, global = true)]
    pub color: ColorArg,

    /// Rendering width.
    #[arg(long, value_name = "N", global = true)]
    pub width: Option<usize>,

    /// Render only repository-relative paths.
    #[arg(long, global = true)]
    pub paths_only: bool,

    /// Output format.
    #[arg(long, value_enum, global = true)]
    pub format: Option<FormatArg>,

    /// Command to run.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// CLI color choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorArg {
    Auto,
    Always,
    Never,
}

/// CLI output format choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum FormatArg {
    Ansi,
    Plain,
    Json,
}

/// gdl subcommands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// Render repository status.
    Status,
}

/// Dispatches a parsed CLI invocation and returns a process exit code.
pub fn run(cli: Cli) -> i32 {
    let command = cli.command.unwrap_or(Command::Status);

    match command {
        Command::Status => run_status(&cli),
    }
}

fn run_status(cli: &Cli) -> i32 {
    let repo_path = match repo_path(cli) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("gdl: {err}");
            return 1;
        }
    };

    let repo = match gdl_core::open(&repo_path) {
        Ok(repo) => repo,
        Err(err) => {
            eprintln!("gdl: {err}");
            return 1;
        }
    };

    let options = options::resolve(cli);
    let output = match gdl_format::status_to_string(&repo, &options) {
        Ok(output) => output,
        Err(err) => {
            eprintln!("gdl: {err}");
            return 1;
        }
    };

    if let Err(err) = io::stdout().write_all(output.as_bytes()) {
        eprintln!("gdl: {err}");
        return 1;
    }

    0
}

fn repo_path(cli: &Cli) -> io::Result<PathBuf> {
    match &cli.repo {
        Some(path) => Ok(path.clone()),
        None => std::env::current_dir(),
    }
}
