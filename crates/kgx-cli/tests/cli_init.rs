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
    for p in [
        "index.md",
        "log.md",
        "CLAUDE.md",
        ".gitignore",
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
        assert!(target.join(p).exists(), "missing {p}");
    }
    // freshly-initialized vault must pass OKF validation
    Command::cargo_bin("kg")
        .unwrap()
        .args(["validate", "--okf"])
        .current_dir(&target)
        .assert()
        .success();
}
