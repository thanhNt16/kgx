use assert_cmd::Command;

#[test]
fn cron_add_writes_unit_file() {
    let home = tempfile::tempdir().unwrap();
    let out = Command::cargo_bin("kg")
        .unwrap()
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path().join(".config"))
        .args([
            "cron",
            "add",
            "dream-nightly",
            "--command",
            "kg dream --max-iterations 3",
            "--calendar",
            "*-*-* 03:00:00",
            "--json",
        ])
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    let written = v["data"]["files"].as_array().unwrap();
    assert!(!written.is_empty());
    assert!(written
        .iter()
        .any(|f| f.as_str().unwrap().contains("dream-nightly")));
}
