use std::path::PathBuf;
use std::time::Instant;
use crate::output::emit;

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
    vault: Option<PathBuf>,
) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = vault.unwrap_or(std::env::current_dir()?);
    std::fs::create_dir_all(&root)?;
    for d in DIRS {
        std::fs::create_dir_all(root.join(d))?;
    }
    let today = kgx_core::util::now_iso();
    let date_prefix = if today.len() >= 10 { &today[..10] } else { &today };
    std::fs::write(
        root.join("index.md"),
        format!("# Knowledge Base Index\n\nokf_version: \"0.1\"\n\n- (add MOCs here)\n"),
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
    let created: Vec<String> = DIRS.iter().map(|s| s.to_string()).collect();
    emit(
        "init",
        serde_json::json!({
            "vault": root.display().to_string(),
            "template": template,
            "dirs": created
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

fn claude_md(template: &str) -> String {
    format!(
        "# CLAUDE.md \u{2014} KGX Agent Contract\n\nokf_version: \"0.1\"\ntemplate: {template}\n\n\
## Note types\nfact | entity | decision | experience | moc | source | question\n\n\
## Conventions\n- One fact per note (Zettelkasten).\n- Provenance: every fact has `source: [[raw/...]]`.\n\
- Supersede, never delete.\n\n(Full Ponytail ladders embedded in Phase 5.)\n"
    )
}
