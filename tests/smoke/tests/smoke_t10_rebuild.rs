/// T10 placeholder: validate command is deterministic across two invocations.
/// Full .kg rebuild determinism lands in Phase 1.
use assert_cmd::Command;

#[test]
fn t10_validate_is_deterministic() {
    let d = tempfile::tempdir().unwrap();
    let vault = d.path().join("vault");

    // Initialize vault
    Command::cargo_bin("kg")
        .unwrap()
        .args(["init", "--okf", "--vault"])
        .arg(&vault)
        .assert()
        .success();

    // Run validate twice and check both are ok
    let out1 = Command::cargo_bin("kg")
        .unwrap()
        .args(["validate", "--okf", "--json"])
        .current_dir(&vault)
        .assert()
        .success();
    let out2 = Command::cargo_bin("kg")
        .unwrap()
        .args(["validate", "--okf", "--json"])
        .current_dir(&vault)
        .assert()
        .success();

    let v1: serde_json::Value = serde_json::from_slice(&out1.get_output().stdout).unwrap();
    let v2: serde_json::Value = serde_json::from_slice(&out2.get_output().stdout).unwrap();

    assert_eq!(v1["ok"], true);
    assert_eq!(v2["ok"], true);
    // Both reports agree on result
    assert_eq!(v1["data"]["ok"], v2["data"]["ok"]);
}
