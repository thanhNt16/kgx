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
    /// Capture raw source (immutable)
    Capture {
        #[arg(long)]
        from: String,
        #[arg(long = "type", default_value = "doc")]
        kind: String,
    },
    /// Extract atomic facts from a source note
    Extract {
        #[arg(long)]
        source: String,
        #[arg(long)]
        batch: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long, default_value = "full")]
        intensity: String,
    },
    /// Analyze and repair links
    Link {
        #[arg(long)]
        suggest: bool,
        #[arg(long)]
        orphans: bool,
        #[arg(long)]
        fix: bool,
    },
    /// Search the knowledge brain
    Search {
        query: String,
        #[arg(long, default_value = "hybrid")]
        mode: String,
        #[arg(long, default_value = "10")]
        limit: usize,
    },
    /// Recall an entity's neighborhood
    Recall {
        #[arg(long)]
        entity: String,
    },
    /// Answer a question using hybrid retrieval
    Ask {
        question: String,
        #[arg(long, default_value = "local")]
        scope: String,
        #[arg(long, default_value = "hybrid")]
        mode: String,
        #[arg(long)]
        cite: bool,
        #[arg(long)]
        write: bool,
    },
}
