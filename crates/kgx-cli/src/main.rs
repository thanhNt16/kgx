mod cli;
mod output;
mod commands {
    pub mod capture;
    pub mod extract_cmd;
    pub mod index;
    pub mod init;
    pub mod link;
    pub mod recall;
    pub mod search;
    pub mod ask;
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
        Commands::Capture { from, kind } => commands::capture::run(cli.json, &from, &kind),
        Commands::Extract {
            source,
            batch,
            dry_run,
            intensity,
        } => commands::extract_cmd::run(cli.json, &source, batch, dry_run, &intensity),
        Commands::Link {
            suggest,
            orphans,
            fix,
        } => commands::link::run(cli.json, suggest, orphans, fix),
        Commands::Search { query, mode, limit } => {
            commands::search::run(cli.json, &query, &mode, limit)
        }
        Commands::Recall { entity } => commands::recall::run(cli.json, &entity),
        Commands::Ask {
            question,
            scope,
            mode,
            cite,
            write,
        } => commands::ask::run(cli.json, &question, &scope, &mode, cite, write),
    }
}
