/// T11 smoke: kg init → kg validate round-trip (OKF partial, Phase 0 slice)
use assert_cmd::Command;
mod common;

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

#[test]
fn t11_ship_pull_validate_roundtrip() {
    let src = common::copy_fixture();
    let bundle = src.path().join("team.okf.tar.gz");
    Command::cargo_bin("kg")
        .unwrap()
        .args(["ship", "--out"])
        .arg(&bundle)
        .current_dir(src.path())
        .assert()
        .success();

    let d = tempfile::tempdir().unwrap();
    let dst = d.path().join("vault");
    Command::cargo_bin("kg")
        .unwrap()
        .args(["init", "--template", "pkm", "--okf", "--vault"])
        .arg(&dst)
        .assert()
        .success();
    Command::cargo_bin("kg")
        .unwrap()
        .args(["pull"])
        .arg(&bundle)
        .args(["--namespace", "team"])
        .current_dir(&dst)
        .assert()
        .success();
    Command::cargo_bin("kg")
        .unwrap()
        .args(["validate", "--okf"])
        .current_dir(&dst)
        .assert()
        .success();
}
