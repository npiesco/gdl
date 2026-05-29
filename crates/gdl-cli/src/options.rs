//! CLI argument resolution into explicit formatter options.

use std::io::{self, IsTerminal};

use gdl_format::{ColorPolicy, OutputFormat, RenderOptions, StatusView};

use crate::{Cli, ColorArg, FormatArg};

/// Resolves parsed CLI arguments into explicit formatter options.
pub fn resolve(args: &Cli) -> RenderOptions {
    let color = resolve_color(args.color);
    let format = resolve_format(args.format, color);

    RenderOptions {
        format,
        color,
        width: resolve_width(args.width),
        view: if args.paths_only {
            StatusView::PathsOnly
        } else {
            StatusView::Full
        },
    }
}

fn resolve_color(color: ColorArg) -> ColorPolicy {
    match color {
        ColorArg::Always => ColorPolicy::Always,
        ColorArg::Never => ColorPolicy::Never,
        ColorArg::Auto => {
            if io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none() {
                ColorPolicy::Always
            } else {
                ColorPolicy::Never
            }
        }
    }
}

fn resolve_format(format: Option<FormatArg>, color: ColorPolicy) -> OutputFormat {
    match format {
        Some(FormatArg::Ansi) => OutputFormat::Ansi,
        Some(FormatArg::Plain) => OutputFormat::Plain,
        Some(FormatArg::Json) => OutputFormat::Json,
        None if color == ColorPolicy::Always => OutputFormat::Ansi,
        None => OutputFormat::Plain,
    }
}

fn resolve_width(width: Option<usize>) -> usize {
    match width {
        Some(width) => width,
        None if io::stdout().is_terminal() => crossterm::terminal::size()
            .map(|(width, _)| usize::from(width))
            .unwrap_or(80),
        None => 80,
    }
}
