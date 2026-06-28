mod cli;
mod git;
mod output;
mod commands {
    pub mod ask;
    pub mod capture;
    pub mod codebase;
    pub mod cron;
    pub mod dashboard;
    pub mod docs;
    pub mod dream_cmd;
    pub mod extract_cmd;
    pub mod graph;
    pub mod index;
    pub mod init;
    pub mod link;
    pub mod mcp_server;
    pub mod pull;
    pub mod recall;
    pub mod review;
    pub mod search;
    pub mod ship;
    pub mod status;
    pub mod sync;
    pub mod tokens;
    pub mod validate;
}

use clap::Parser;
use cli::{Cli, Commands, DocsCommand};

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
            with_skills,
            with_rtk,
            vault,
        } => commands::init::run(cli.json, &template, okf, with_skills, with_rtk, vault),
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
        Commands::Search {
            query,
            mode,
            limit,
            rerank_graph,
            rerank_llm,
        } => commands::search::run(cli.json, &query, &mode, limit, rerank_graph, rerank_llm),
        Commands::Recall { entity } => commands::recall::run(cli.json, &entity),
        Commands::Ask {
            question,
            scope,
            mode,
            cite,
            write,
        } => commands::ask::run(cli.json, &question, &scope, &mode, cite, write),
        Commands::Dream {
            max_iterations,
            only,
            dry_run,
        } => commands::dream_cmd::run(cli.json, max_iterations, only, dry_run),
        Commands::Review {
            approve,
            reject,
            interactive,
            ponytail_audit,
        } => commands::review::run(cli.json, approve, reject, interactive, ponytail_audit),
        Commands::McpServer { transport } => commands::mcp_server::run(cli.json, &transport),
        Commands::Cron {
            action,
            name,
            command,
            calendar,
        } => commands::cron::run(cli.json, &action, name, command, calendar),
        Commands::Graph {
            format,
            out,
            filter,
        } => commands::graph::run(cli.json, &format, out, filter),
        Commands::Docs { command } => match command {
            DocsCommand::Usecase { name, out } => commands::docs::run_usecase(cli.json, &name, out),
        },
        Commands::Status { verbose } => commands::status::run(cli.json, verbose),
        Commands::Tokens { since, by } => commands::tokens::run(cli.json, &since, &by),
        Commands::Dashboard => commands::dashboard::run(cli.json),
        Commands::Ship { out } => commands::ship::run(cli.json, out),
        Commands::Pull { file, namespace } => commands::pull::run(cli.json, file, namespace),
        Commands::Sync { action } => commands::sync::run(cli.json, &action),
        Commands::Codebase { command } => commands::codebase::run(cli.json, command),
    }
}
