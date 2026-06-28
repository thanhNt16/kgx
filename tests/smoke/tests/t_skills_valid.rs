#[test]
fn native_skill_packages_reference_same_mcp_tools() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let files = [
        root.join("skills/claude/.claude/skills/kgx/SKILL.md"),
        root.join("skills/codex/AGENTS.md"),
        root.join("skills/cursor/.cursor/rules/kgx.mdc"),
        root.join("skills/opencode/.opencode/skills/kgx/SKILL.md"),
    ];
    let tools = [
        "search_notes",
        "get_note",
        "upsert_note",
        "ask_question",
        "capture_raw",
        "dream_step",
    ];
    for file in files {
        let text = std::fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", file.display()));
        for tool in tools {
            assert!(text.contains(tool), "{} missing {tool}", file.display());
        }
    }
    for config in [
        root.join("skills/claude/.mcp.json"),
        root.join("skills/codex/config.toml"),
        root.join("skills/cursor/.cursor/mcp.json"),
        root.join("skills/opencode/opencode.json"),
    ] {
        let text = std::fs::read_to_string(&config)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", config.display()));
        assert!(text.contains("mcp-server"));
        assert!(text.contains("stdio"));
    }
    for hook in [
        root.join("skills/claude/.claude/settings.json"),
        root.join("skills/codex/hooks.json"),
        root.join("skills/opencode/.opencode/plugins/kgx-verify-finished.js"),
        root.join("skills/hooks/verify-finished.sh"),
    ] {
        let text = std::fs::read_to_string(&hook)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", hook.display()));
        assert!(
            text.contains("verify-finished"),
            "{} missing shared finish hook reference",
            hook.display()
        );
    }
}
