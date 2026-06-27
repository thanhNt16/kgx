mod cli;
mod output;
mod commands {
    pub mod index;
    pub mod init;
    pub mod validate;
}

use clap::Parser;
use cli::{Cli, Commands};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Validate {
            okf,
            links,
            frontmatter,
            bitemporal,
        } => commands::validate::run(cli.json, okf, links, frontmatter, bitemporal),
        Commands::Init {
            template,
            okf,
            vault,
        } => commands::init::run(cli.json, &template, okf, vault),
        Commands::Index {
            full,
            incremental,
            pagerank,
            communities,
        } => commands::index::run(cli.json, full, incremental, pagerank, communities),
    }
}
