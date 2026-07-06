//! Vault root resolution.
//!
//! A KGX vault lives in a `.brain/` directory. Knowledge content (`raw/`,
//! `notes/`, `index.md`, `log.md`, `.kg/`, `CLAUDE.md`) is stored *inside*
//! `.brain/`; agent/tooling config (`.mcp.json`, `.claude/`, `.codex/`,
//! `.cursor/`, `.opencode/`, `.kgx/`, `AGENTS.md`, `config.toml`,
//! `opencode.json`, `.gitignore`) lives at the enclosing project root so that
//! agents can discover it.
//!
//! `vault_root()` resolves the `.brain/` directory for commands that operate on
//! knowledge content. `project_root()` resolves the enclosing directory (where
//! agent config and `.brain/` itself live). Both use the process current
//! directory as the anchor — matching the prior `std::env::current_dir()`
//! semantics — so callers should `cd` into the project before running `kg`.

use std::path::PathBuf;

/// Name of the knowledge-vault directory inside a project.
pub const BRAIN_DIR: &str = ".brain";

/// The enclosing project directory: where agent config lives and where
/// `.brain/` itself resides. Equivalent to the process current directory.
#[allow(dead_code)]
pub fn project_root() -> anyhow::Result<PathBuf> {
    Ok(std::env::current_dir()?)
}

/// The knowledge-vault root: `<project>/.brain`.
///
/// Errors with a remediation hint if `.brain/` is missing, so running a
/// knowledge command outside an initialized vault fails loudly instead of
/// silently reading an empty cwd. Commands that need the project root (init,
/// project, codebase, cron, docs, sync) should use [`project_root`] instead.
pub fn vault_root() -> anyhow::Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let brain = cwd.join(BRAIN_DIR);
    if brain.is_dir() {
        Ok(brain)
    } else {
        anyhow::bail!(
            "no {BRAIN_DIR}/ vault in {} — run `kg init` (or `kg init --migrate` for a legacy root-level vault)",
            cwd.display()
        )
    }
}
