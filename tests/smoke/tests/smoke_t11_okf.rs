/// T11 smoke: kg init → kg validate round-trip (OKF partial, Phase 0 slice)
use assert_cmd::Command;

#[test]
fn t11_init_then_validate_passes() {
    let d = tempfile::tempdir().unwrap();
    let vault = d.path().join("vault");

    // kg init must succeed
    Command::cargo_bin("kg")
        .unwrap()
        .args(["init", "--template", "pkm", "--okf", "--vault"])
        .arg(&vault)
        .assert()
        .success();

    // kg validate --okf --json must report ok: true
    let out = Command::cargo_bin("kg")
        .unwrap()
        .args(["validate", "--okf", "--json"])
        .current_dir(&vault)
        .assert()
        .success();

    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["ok"], true, "validate should report ok=true after init");
    assert_eq!(v["command"], "validate");
}
