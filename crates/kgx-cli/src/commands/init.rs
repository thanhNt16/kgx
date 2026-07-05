use crate::output::emit;
use std::path::PathBuf;
use std::time::Instant;

const DIRS: &[&str] = &[
    "raw/assets",
    "notes/facts",
    "notes/entities",
    "notes/decisions",
    "notes/experiences",
    "notes/moc",
    "notes/sources",
    "notes/questions",
    "notes/archived",
];

pub fn run(
    json: bool,
    template: &str,
    _okf: bool,
    with_skills: bool,
    with_rtk: bool,
    vault: Option<PathBuf>,
) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = vault.unwrap_or(std::env::current_dir()?);
    std::fs::create_dir_all(&root)?;
    for d in DIRS {
        std::fs::create_dir_all(root.join(d))?;
    }
    let today = kgx_core::util::now_iso();
    let date_prefix = if today.len() >= 10 {
        &today[..10]
    } else {
        &today
    };
    std::fs::write(
        root.join("index.md"),
        "# Knowledge Base Index\n\nokf_version: \"0.1\"\n\n- (add MOCs here)\n",
    )?;
    std::fs::write(
        root.join("log.md"),
        format!(
            "# Log\n\n## [{}] init | template={}\n",
            date_prefix, template
        ),
    )?;
    std::fs::write(root.join("CLAUDE.md"), claude_md(template))?;
    std::fs::write(root.join(".gitignore"), ".kg/\n.obsidian/workspace*\n")?;
    if with_skills {
        write_skills(&root)?;
    }
    if with_rtk {
        kgx_rtk::install_hooks(kgx_rtk::Tool::ClaudeCode, &root)?;
        kgx_rtk::install_hooks(kgx_rtk::Tool::Codex, &root)?;
        kgx_rtk::install_hooks(kgx_rtk::Tool::Cursor, &root)?;
        kgx_rtk::install_hooks(kgx_rtk::Tool::Opencode, &root)?;
    }
    let created: Vec<String> = DIRS.iter().map(|s| s.to_string()).collect();
    emit(
        "init",
        serde_json::json!({
            "vault": root.display().to_string(),
            "template": template,
            "dirs": created,
            "with_skills": with_skills,
            "with_rtk": with_rtk
        }),
        json,
        start,
        |_| {
            println!(
                "\u{2714} initialized vault at {} (template: {template})",
                root.display()
            );
        },
    );
    Ok(())
}

fn write_skills(root: &std::path::Path) -> anyhow::Result<()> {
    const FILES: &[(&str, &str)] = &[
        (
            ".mcp.json",
            include_str!("../../../../skills/claude/.mcp.json"),
        ),
        (
            ".claude/skills/kgx/SKILL.md",
            include_str!("../../../../skills/claude/.claude/skills/kgx/SKILL.md"),
        ),
        (
            ".claude/settings.json",
            include_str!("../../../../skills/claude/.claude/settings.json"),
        ),
        (
            "AGENTS.md",
            include_str!("../../../../skills/codex/AGENTS.md"),
        ),
        (
            "config.toml",
            include_str!("../../../../skills/codex/config.toml"),
        ),
        (
            ".codex/hooks.json",
            include_str!("../../../../skills/codex/hooks.json"),
        ),
        (
            ".kgx/hooks/verify-finished.sh",
            include_str!("../../../../skills/hooks/verify-finished.sh"),
        ),
        (
            ".cursor/mcp.json",
            include_str!("../../../../skills/cursor/.cursor/mcp.json"),
        ),
        (
            ".cursor/rules/kgx.mdc",
            include_str!("../../../../skills/cursor/.cursor/rules/kgx.mdc"),
        ),
        (
            "opencode.json",
            include_str!("../../../../skills/opencode/opencode.json"),
        ),
        (
            ".opencode/skills/kgx/SKILL.md",
            include_str!("../../../../skills/opencode/.opencode/skills/kgx/SKILL.md"),
        ),
        (
            ".opencode/plugins/kgx-verify-finished.js",
            include_str!("../../../../skills/opencode/.opencode/plugins/kgx-verify-finished.js"),
        ),
        (
            ".claude/skills/kgx-codebase/SKILL.md",
            include_str!("../../../../skills/claude/.claude/skills/kgx-codebase/SKILL.md"),
        ),
        (
            ".opencode/skills/kgx-codebase/SKILL.md",
            include_str!("../../../../skills/opencode/.opencode/skills/kgx-codebase/SKILL.md"),
        ),
        (
            ".claude/skills/kgx-codebase-index/SKILL.md",
            include_str!("../../../../skills/claude/.claude/skills/kgx-codebase-index/SKILL.md"),
        ),
        (
            ".opencode/skills/kgx-codebase-index/SKILL.md",
            include_str!(
                "../../../../skills/opencode/.opencode/skills/kgx-codebase-index/SKILL.md"
            ),
        ),
    ];
    for (rel, content) in FILES {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
    }
    write_commands(root)?;
    Ok(())
}

fn write_commands(root: &std::path::Path) -> anyhow::Result<()> {
    const COMMANDS: &[(&str, &str, &str)] = &[
        (
            "ingest",
            "Capture a source (file/URL/conversation) and extract atomic facts",
            "Capture a source and extract atomic facts.\n\
             \n\
             1. Ask the user what to ingest: a file path, URL, or conversation text\n\
             2. Use the appropriate MCP tool:\n\
                - `ingest_file` for a file (pass content as text)\n\
                - `ingest_url` for a URL\n\
                - `ingest_conversation` for a conversation (pass each turn as {role, content})\n\
             3. After ingest, run `kg extract --source <returned_id> --intensity full` via Bash\n\
             4. Summarize what was captured and extracted",
        ),
        (
            "capture",
            "Capture raw source material verbatim (immutable)",
            "Capture raw source material.\n\
             \n\
             1. Ask the user what to capture: a file path, URL, or pasted content\n\
             2. Use the appropriate MCP tool:\n\
                - `ingest_file` for file content\n\
                - `ingest_url` for a URL\n\
                - `ingest_conversation` for conversation text\n\
             3. Show the captured source ID to the user",
        ),
        (
            "extract",
            "Extract atomic facts, entities, and decisions from a captured source",
            "Extract atomic facts from a source.\n\
             \n\
             1. Ask the user for the source note ID (or find it via `query_memory`)\n\
             2. Run `kg extract --source <source_id> --intensity full` via Bash\n\
             3. Show the extracted facts, entities, and decisions",
        ),
        (
            "index",
            "Build or rebuild the SQLite brain index with communities",
            "Build or rebuild the brain index.\n\
             \n\
             1. Run `kg index --full --communities` via Bash\n\
             2. Wait for completion\n\
             3. Show the indexing result (note count, community count, duration)",
        ),
        (
            "search",
            "Hybrid keyword + semantic search over the knowledge graph",
            "Search the knowledge graph.\n\
             \n\
             1. Ask the user for their search query\n\
             2. Use the `nl_query_memory` or `deep_search_memory` MCP tool with the query\n\
             3. Show results with their note IDs and relevance",
        ),
        (
            "ask",
            "Ask a question with citation-backed answers from the knowledge graph",
            "Ask a question using the knowledge graph.\n\
             \n\
             1. Ask the user for their question\n\
             2. Use `nl_query_memory` MCP tool with the query\n\
             3. Present the answer with citations to source note IDs",
        ),
        (
            "recall",
            "Retrieve notes within 1-2 hops of a named entity",
            "Retrieve entity neighborhood.\n\
             \n\
             1. Ask the user for an entity name\n\
             2. Run `kg recall --entity \"<entity_name>\"` via Bash\n\
             3. Show the notes within 1-2 hops of the entity",
        ),
        (
            "dream",
            "Run dream consolidation (dedup, contradiction, supersession, stale archival)",
            "Run full dream consolidation.\n\
             \n\
             1. Run `kg dream --max-iterations 3` via Bash\n\
             2. Show the staged diffs to the user\n\
             3. Ask user to approve and run `kg review --approve all --ponytail-audit` via Bash",
        ),
        (
            "review",
            "Review staged dream diffs without running consolidation",
            "Review staged dream diffs.\n\
             \n\
             1. Run `kg review` via Bash to show staged diffs\n\
             2. Ask the user whether to approve or reject\n\
             3. If approve: run `kg review --approve all --ponytail-audit` via Bash\n\
             4. If reject: inform the user and suggest `kg dream` to regenerate",
        ),
        (
            "link",
            "Analyze and repair broken wikilinks in the vault",
            "Analyze and repair wikilinks.\n\
             \n\
             1. Run `kg link` via Bash to show broken links\n\
             2. Ask the user if they want to auto-fix\n\
             3. If yes: run `kg link --fix` via Bash\n\
             4. Show the repaired links",
        ),
        (
            "status",
            "Show vault structure, brain size, and index freshness",
            "Show vault and brain status.\n\
             \n\
             1. Run `kg status` via Bash\n\
             2. Show vault structure, brain size, and index info\n\
             3. If `--json` is preferred, run `kg status --json` and format nicely",
        ),
        (
            "init",
            "Scaffold a new KGX vault with templates and skills",
            "Scaffold a new KGX vault.\n\
             \n\
             1. Ask the user for:\n\
                - Vault path (default: current directory or choose)\n\
                - Template: research, code, pkm, or team\n\
                - Whether to include skills (`--with-skills`)\n\
                - Whether to include RTK (`--with-rtk`)\n\
             2. Run `kg init [--template <type>] [--with-skills] [--with-rtk] [--vault <path>]` via Bash\n\
             3. Show the created directory structure",
        ),
        (
            "ship",
            "Create a portable OKF bundle for sharing the vault",
            "Create an OKF bundle.\n\
             \n\
             1. Ask the user for version (semver) and bundle name\n\
             2. Run `kg ship --version <version> --name \"<name>\"` via Bash\n\
             3. Show the created bundle path and contents",
        ),
        (
            "sync",
            "Pull remote changes and reindex the brain",
            "Pull remote changes and reindex.\n\
             \n\
             1. Run `kg sync` via Bash\n\
             2. Show the pulled changes and reindex status",
        ),
    ];
    let cmds_dir = root.join(".claude/commands");
    std::fs::create_dir_all(&cmds_dir)?;
    for (verb, description, body) in COMMANDS {
        let content = format!(
            "---\nname: kgx:{verb}\ndescription: {description}\ndisable-model-invocation: true\n---\n\n# kgx:{verb}\n\n{body}\n"
        );
        std::fs::write(cmds_dir.join(format!("kgx:{verb}.md")), content)?;
    }
    Ok(())
}

fn claude_md(template: &str) -> String {
    format!(
        "# CLAUDE.md \u{2014} KGX Agent Contract\n\nokf_version: \"0.1\"\ntemplate: {template}\n\n\
## Note types\nfact | entity | decision | experience | moc | source | question\n\n\
## Conventions\n- One fact per note (Zettelkasten).\n- Provenance: every fact has `source: [[raw/...]]`.\n\
- Supersede, never delete.\n\n(Full Ponytail ladders embedded in Phase 5.)\n"
    )
}
