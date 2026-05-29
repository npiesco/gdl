//! gdl CLI entrypoint.

use clap::Parser;

fn main() {
    let cli = gdl_cli::Cli::parse();
    std::process::exit(gdl_cli::run(cli));
}
