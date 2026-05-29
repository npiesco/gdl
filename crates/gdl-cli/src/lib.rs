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

/// CLI diff area choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AreaArg {
    Worktree,
    Staged,
    Head,
}

impl From<AreaArg> for gdl_core::DiffArea {
    fn from(area: AreaArg) -> Self {
        match area {
            AreaArg::Worktree => Self::Worktree,
            AreaArg::Staged => Self::Staged,
            AreaArg::Head => Self::Head,
        }
    }
}

/// gdl subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// Render repository status.
    Status,
    /// Render a file diff.
    Diff {
        /// Repository-relative file path to diff.
        path: PathBuf,

        /// Repository area to compare.
        #[arg(long, value_enum, default_value_t = AreaArg::Worktree)]
        area: AreaArg,
    },
}

/// Dispatches a parsed CLI invocation and returns a process exit code.
pub fn run(cli: Cli) -> i32 {
    let command = cli.command.clone().unwrap_or(Command::Status);

    match command {
        Command::Status => run_status(&cli),
        Command::Diff { path, area } => run_diff(&cli, path, area.into()),
    }
}

fn run_status(cli: &Cli) -> i32 {
    let repo = match open_repo(cli) {
        Ok(repo) => repo,
        Err(code) => return code,
    };

    let options = options::resolve(cli);
    let output = match gdl_format::status_to_string(&repo, &options) {
        Ok(output) => output,
        Err(err) => {
            eprintln!("gdl: {err}");
            return 1;
        }
    };

    write_output(output)
}

fn run_diff(cli: &Cli, path: PathBuf, area: gdl_core::DiffArea) -> i32 {
    let repo = match open_repo(cli) {
        Ok(repo) => repo,
        Err(code) => return code,
    };

    let options = options::resolve(cli);
    let output = match gdl_format::diff_to_string(&repo, path, &options, area) {
        Ok(output) => output,
        Err(err) => {
            eprintln!("gdl: {err}");
            return 1;
        }
    };

    write_output(output)
}

fn open_repo(cli: &Cli) -> Result<gdl_core::Repository, i32> {
    let repo_path = match repo_path(cli) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("gdl: {err}");
            return Err(1);
        }
    };

    let repo = match gdl_core::open(&repo_path) {
        Ok(repo) => repo,
        Err(err) => {
            eprintln!("gdl: cannot open repo at {}: {err}", repo_path.display());
            return Err(1);
        }
    };

    Ok(repo)
}

fn write_output(output: String) -> i32 {
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
