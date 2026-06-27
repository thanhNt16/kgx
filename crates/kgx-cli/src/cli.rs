use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "kg", version, about = "Local-first AI-managed knowledge graph")]
pub struct Cli {
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Validate vault integrity and OKF conformance
    Validate {
        #[arg(long)]
        okf: bool,
        #[arg(long)]
        links: bool,
        #[arg(long)]
        frontmatter: bool,
        #[arg(long)]
        bitemporal: bool,
    },
    /// Scaffold a new OKF vault
    Init {
        #[arg(long, default_value = "pkm")]
        template: String,
        #[arg(long)]
        okf: bool,
        #[arg(long)]
        vault: Option<std::path::PathBuf>,
    },
    /// Build/refresh .kg/brain.sqlite
    Index {
        #[arg(long)]
        full: bool,
        #[arg(long)]
        incremental: bool,
        #[arg(long)]
        pagerank: bool,
        #[arg(long)]
        communities: bool,
    },
}
