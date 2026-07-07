use assert_cmd::Command;

#[test]
fn init_creates_valid_okf_vault() {
    let d = tempfile::tempdir().unwrap();
    let target = d.path().join("brain");
    Command::cargo_bin("kg")
        .unwrap()
        .args(["init", "--template", "research", "--okf", "--vault"])
        .arg(&target)
        .assert()
        .success();
    // Knowledge content lives under <project>/.brain/.
    for p in [
        "index.md",
        "log.md",
        "CLAUDE.md",
        "notes/facts",
        "notes/entities",
        "notes/decisions",
        "notes/moc",
        "notes/questions",
        "notes/sources",
        "notes/experiences",
        "notes/archived",
        "raw",
    ] {
        assert!(target.join(".brain").join(p).exists(), "missing .brain/{p}");
    }
    // The .gitignore lives at the project root and ignores derived brain state.
    assert!(
        target.join(".gitignore").exists(),
        "missing .gitignore at root"
    );
    let ignore = std::fs::read_to_string(target.join(".gitignore")).unwrap();
    assert!(
        ignore.contains(".brain/.kg/"),
        ".gitignore must ignore .brain/.kg/, got: {ignore}"
    );
    // freshly-initialized vault must pass OKF validation
    Command::cargo_bin("kg")
        .unwrap()
        .args(["validate", "--okf"])
        .current_dir(&target)
        .assert()
        .success();
}

#[test]
fn init_with_skills_and_rtk_writes_tool_artifacts() {
    let d = tempfile::tempdir().unwrap();
    let target = d.path().join("brain");
    Command::cargo_bin("kg")
        .unwrap()
        .args([
            "init",
            "--template",
            "research",
            "--with-skills",
            "--with-rtk",
            "--vault",
        ])
        .arg(&target)
        .assert()
        .success();

    // Agent/tooling config lives at the project root (NOT under .brain/).
    for p in [
        ".mcp.json",
        ".claude/skills/kgx/SKILL.md",
        "AGENTS.md",
        "config.toml",
        ".cursor/mcp.json",
        ".cursor/rules/kgx.mdc",
        ".claude/settings.json",
        ".codex/hooks.json",
        ".kgx/hooks/verify-finished.sh",
        ".codex/rtk.toml",
        ".cursor/rtk.json",
        "opencode.json",
        ".opencode/skills/kgx/SKILL.md",
        ".opencode/plugins/kgx-verify-finished.js",
        ".opencode/rtk.md",
    ] {
        assert!(target.join(p).exists(), "missing {p} at project root");
        assert!(
            !target.join(".brain").join(p).exists(),
            "{p} must NOT live under .brain/"
        );
    }
}

/// A legacy root-level vault is relocated into .brain/ by `kg init --migrate`,
/// and knowledge commands continue to see the same notes afterwards.
#[test]
fn init_migrate_moves_legacy_vault_into_brain() {
    let d = tempfile::tempdir().unwrap();
    let target = d.path().join("brain");
    // Build a legacy root-level vault: notes/ + .kg/ directly at the root.
    std::fs::create_dir_all(target.join("notes/facts")).unwrap();
    std::fs::write(
        target.join("notes/facts/f-legacy.md"),
        "---\ntype: fact\nid: legacy-1\ntitle: Legacy\n---\nold layout\n",
    )
    .unwrap();
    std::fs::create_dir_all(target.join(".kg")).unwrap();
    std::fs::write(target.join(".kg/meta.json"), "{}").unwrap();
    std::fs::write(target.join("index.md"), "# legacy\n").unwrap();

    Command::cargo_bin("kg")
        .unwrap()
        .args(["init", "--migrate", "--vault"])
        .arg(&target)
        .assert()
        .success();

    // Legacy items moved under .brain/.
    assert!(target.join(".brain/notes/facts/f-legacy.md").exists());
    assert!(target.join(".brain/.kg/meta.json").exists());
    assert!(target.join(".brain/index.md").exists());
    // And gone from the root.
    assert!(!target.join("notes").exists());
    assert!(!target.join(".kg").exists());
    assert!(!target.join("index.md").exists());

    // `kg status` resolves .brain/ and still sees the note.
    Command::cargo_bin("kg")
        .unwrap()
        .args(["status", "--json"])
        .current_dir(&target)
        .assert()
        .success();
}
