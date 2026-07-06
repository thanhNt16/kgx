use crate::output::emit;
use crate::vault::BRAIN_DIR;
use std::path::{Path, PathBuf};
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

/// Legacy root-level vault members that `--migrate` relocates into `.brain/`.
/// Stored `rel_path`s are recorded relative to the vault root (e.g.
/// `notes/facts/...`), so a whole-tree rename preserves them.
const LEGACY_VAULT_MEMBERS: &[&str] = &["raw", "notes", ".kg", "index.md", "log.md", "CLAUDE.md"];

pub fn run(
    json: bool,
    template: &str,
    _okf: bool,
    with_skills: bool,
    with_rtk: bool,
    vault: Option<PathBuf>,
    migrate: bool,
) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = vault.unwrap_or(std::env::current_dir()?);
    std::fs::create_dir_all(&root)?;

    if migrate {
        return run_migrate(&root, template, with_skills, with_rtk, json, start);
    }

    // Knowledge content lives inside <root>/.brain/. Agent/tooling config
    // (.mcp.json, .claude/, .codex/, .cursor/, .opencode/, .kgx/, AGENTS.md,
    // config.toml, opencode.json) lives at <root> so agents can discover it.
    let brain = root.join(BRAIN_DIR);
    scaffold(&brain, template)?;
    write_gitignore(&root)?;
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
            "project": root.display().to_string(),
            "vault": brain.display().to_string(),
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
                brain.display()
            );
        },
    );
    Ok(())
}

/// Create the knowledge-vault skeleton (raw/, notes/, index.md, log.md,
/// CLAUDE.md) inside `brain`.
fn scaffold(brain: &Path, template: &str) -> anyhow::Result<()> {
    std::fs::create_dir_all(brain)?;
    for d in DIRS {
        std::fs::create_dir_all(brain.join(d))?;
    }
    let today = kgx_core::util::now_iso();
    let date_prefix = if today.len() >= 10 {
        &today[..10]
    } else {
        &today
    };
    std::fs::write(
        brain.join("index.md"),
        "# Knowledge Base Index\n\nokf_version: \"0.1\"\n\n- (add MOCs here)\n",
    )?;
    std::fs::write(
        brain.join("log.md"),
        format!("# Log\n\n## [{}] init | template={}\n", date_prefix, template),
    )?;
    std::fs::write(brain.join("CLAUDE.md"), claude_md(template))?;
    Ok(())
}

/// Project-level `.gitignore`. `.brain/.kg/` holds derived state (brain.sqlite,
/// embeddings, staged diffs) that should not be committed.
fn write_gitignore(root: &Path) -> anyhow::Result<()> {
    std::fs::write(
        root.join(".gitignore"),
        ".brain/.kg/\n.obsidian/workspace*\n",
    )?;
    Ok(())
}

/// Relocate a legacy root-level vault into `.brain/`.
fn run_migrate(
    root: &Path,
    template: &str,
    with_skills: bool,
    with_rtk: bool,
    json: bool,
    start: Instant,
) -> anyhow::Result<()> {
    let brain = root.join(BRAIN_DIR);
    let brain_exists = brain.is_dir();
    let has_legacy = LEGACY_VAULT_MEMBERS
        .iter()
        .any(|m| root.join(m).exists());

    if brain_exists && !has_legacy {
        anyhow::bail!(
            "{} already exists and no legacy vault members were found at {} — nothing to migrate",
            brain.display(),
            root.display()
        );
    }
    if !has_legacy {
        anyhow::bail!(
            "no legacy vault found at {} — run `kg init` to create a new vault",
            root.display()
        );
    }

    std::fs::create_dir_all(&brain)?;
    let mut moved: Vec<String> = Vec::new();
    for m in LEGACY_VAULT_MEMBERS {
        let src = root.join(m);
        if !src.exists() {
            continue;
        }
        let dst = brain.join(m);
        if dst.exists() {
            // Don't clobber an existing migrated copy.
            eprintln!("skip {} (already present in {BRAIN_DIR}/)", m);
            continue;
        }
        // Same filesystem → atomic. Create the parent so rename succeeds even
        // for nested legacy layouts (e.g. a stray `.kg` without `.brain`).
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&src, &dst)?;
        moved.push((*m).to_string());
    }

    // Ensure the new vault has a complete skeleton even if the legacy one was
    // partial, and refresh the project .gitignore.
    scaffold(&brain, template)?;
    write_gitignore(root)?;
    if with_skills {
        write_skills(root)?;
    }
    if with_rtk {
        kgx_rtk::install_hooks(kgx_rtk::Tool::ClaudeCode, root)?;
        kgx_rtk::install_hooks(kgx_rtk::Tool::Codex, root)?;
        kgx_rtk::install_hooks(kgx_rtk::Tool::Cursor, root)?;
        kgx_rtk::install_hooks(kgx_rtk::Tool::Opencode, root)?;
    }
    emit(
        "init",
        serde_json::json!({
            "migrated": true,
            "project": root.display().to_string(),
            "vault": brain.display().to_string(),
            "moved": moved,
            "template": template,
            "with_skills": with_skills,
            "with_rtk": with_rtk
        }),
        json,
        start,
        |_| {
            println!(
                "\u{2714} migrated vault into {} (template: {template})",
                brain.display()
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
    const CMDS: &[(&str, &str)] = &[
        (
            "ask",
            include_str!("../../../../skills/claude/.claude/commands/ask.md"),
        ),
        (
            "capture",
            include_str!("../../../../skills/claude/.claude/commands/capture.md"),
        ),
        (
            "dream",
            include_str!("../../../../skills/claude/.claude/commands/dream.md"),
        ),
        (
            "extract",
            include_str!("../../../../skills/claude/.claude/commands/extract.md"),
        ),
        (
            "index",
            include_str!("../../../../skills/claude/.claude/commands/index.md"),
        ),
        (
            "ingest",
            include_str!("../../../../skills/claude/.claude/commands/ingest.md"),
        ),
        (
            "init",
            include_str!("../../../../skills/claude/.claude/commands/init.md"),
        ),
        (
            "link",
            include_str!("../../../../skills/claude/.claude/commands/link.md"),
        ),
        (
            "recall",
            include_str!("../../../../skills/claude/.claude/commands/recall.md"),
        ),
        (
            "review",
            include_str!("../../../../skills/claude/.claude/commands/review.md"),
        ),
        (
            "search",
            include_str!("../../../../skills/claude/.claude/commands/search.md"),
        ),
        (
            "ship",
            include_str!("../../../../skills/claude/.claude/commands/ship.md"),
        ),
        (
            "status",
            include_str!("../../../../skills/claude/.claude/commands/status.md"),
        ),
        (
            "sync",
            include_str!("../../../../skills/claude/.claude/commands/sync.md"),
        ),
    ];
    let cmds_dir = root.join(".claude/commands");
    std::fs::create_dir_all(&cmds_dir)?;
    for (verb, content) in CMDS {
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
