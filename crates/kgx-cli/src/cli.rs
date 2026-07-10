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
        with_skills: bool,
        #[arg(long)]
        with_rtk: bool,
        #[arg(long)]
        vault: Option<std::path::PathBuf>,
        /// Migrate a legacy root-level vault into .brain/
        #[arg(long)]
        migrate: bool,
    },
    /// Build/refresh .kg/brain.sqlite
    Index {
        #[arg(long)]
        full: bool,
        #[arg(long)]
        incremental: bool,
        #[arg(long)]
        rebuild_vectors: bool,
        #[arg(long)]
        pagerank: bool,
    },
    /// Capture raw source (immutable)
    Capture {
        #[arg(long)]
        from: String,
        #[arg(long = "type", default_value = "doc")]
        kind: String,
        /// Comma-separated extensions to capture when --from is a directory
        /// (default: md,txt,markdown,mdx,pdf,docx,pptx,odt,epub,html,htm,xlsx,xls)
        #[arg(long)]
        ext: Option<String>,
        /// URL crawl depth: 0 = single page, 1 = page + direct links, etc.
        #[arg(long, default_value = "0")]
        depth: u32,
        /// Maximum pages to fetch during URL crawl
        #[arg(long, default_value = "50")]
        max_pages: u32,
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
        /// Use two-stage retrieve → graph rerank pipeline instead of fused RRF
        #[arg(long)]
        rerank_graph: bool,
    },
    /// Recall an entity's neighborhood
    Recall {
        #[arg(long)]
        entity: String,
        /// Include typed relationship edges in the output
        #[arg(long)]
        relations: bool,
    },
    /// Query notes with filters
    Query {
        #[arg(long)]
        note_type: Option<String>,
        #[arg(long)]
        entity_type: Option<String>,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long, default_value = "20")]
        limit: usize,
    },
    /// Review staged dream diffs
    Review {
        #[arg(long)]
        approve: Option<String>,
        #[arg(long)]
        reject: Option<String>,
        #[arg(long)]
        interactive: bool,
        #[arg(long)]
        ponytail_audit: bool,
    },
    /// Start the MCP server
    McpServer {
        #[arg(long, default_value = "stdio")]
        transport: String,
    },
    /// Manage KGX scheduler jobs
    Cron {
        action: String,
        name: Option<String>,
        #[arg(long)]
        command: Option<String>,
        #[arg(long, alias = "schedule")]
        calendar: Option<String>,
    },
    /// Export the knowledge graph
    Graph {
        #[arg(long, default_value = "html")]
        format: String,
        #[arg(long)]
        out: Option<std::path::PathBuf>,
        #[arg(long)]
        filter: Option<String>,
    },
    /// Generate documentation artifacts
    Docs {
        #[command(subcommand)]
        command: DocsCommand,
    },
    /// Manage KGX projects (per-project brains)
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Print vault and brain status
    Status {
        #[arg(long)]
        verbose: bool,
    },
    /// Print token accounting summaries
    Tokens {
        #[arg(long, default_value = "30d")]
        since: String,
        #[arg(long, default_value = "operation")]
        by: String,
    },
    /// Print or display a dashboard snapshot
    Dashboard,
    /// Create an OKF bundle
    Ship {
        #[arg(long)]
        out: std::path::PathBuf,
    },
    /// Import an OKF bundle
    Pull {
        file: std::path::PathBuf,
        #[arg(long)]
        namespace: Option<String>,
    },
    /// Start the HTTP/stdio server
    Serve {
        #[arg(long, default_value = "stdio")]
        transport: String,
        #[arg(long, default_value_t = 8765)]
        port: u16,
    },
    /// Sync vault changes through git
    Sync { action: String },
    /// Codebase graph operations via codebase-memory-mcp
    Codebase {
        #[command(subcommand)]
        command: CodebaseCommand,
    },
}

#[derive(Subcommand)]
pub enum CodebaseCommand {
    /// Install codebase-memory-mcp binary
    Install,
    /// Index the codebase into the graph
    Index {
        #[arg(long)]
        path: Option<std::path::PathBuf>,
    },
    /// Search the codebase graph
    Search {
        query: String,
        #[arg(long, default_value = "10")]
        limit: usize,
    },
    /// Trace call paths for a function
    Trace {
        function: String,
        #[arg(long, default_value = "inbound")]
        direction: String,
    },
    /// Show architecture overview
    Architecture,
    /// Show indexing status
    Status,
    /// Update codebase-memory-mcp binary
    Update,
}

#[derive(Subcommand)]
pub enum ProjectCommand {
    /// Add a project (create its brain)
    Add { name: String },
    /// List registered projects
    List,
    /// Switch active project
    Use { name: String },
    /// Remove a project
    Remove { name: String },
}

#[derive(Subcommand)]
pub enum DocsCommand {
    /// Render a use-case walkthrough
    Usecase {
        name: String,
        #[arg(long)]
        out: std::path::PathBuf,
    },
}
