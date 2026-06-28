use assert_cmd::Command;
mod common;

#[test]
fn status_json_reports_counts_and_orphans() {
    let d = common::copy_fixture();
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full"])
        .current_dir(d.path())
        .assert()
        .success();
    let out = Command::cargo_bin("kg")
        .unwrap()
        .args(["status", "--json"])
        .current_dir(d.path())
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["data"]["nodes"], 17);
    assert_eq!(v["data"]["orphans"], 1);
}

#[test]
fn tokens_by_operation_json() {
    let d = common::copy_fixture();
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full"])
        .current_dir(d.path())
        .assert()
        .success();
    let out = Command::cargo_bin("kg")
        .unwrap()
        .args(["tokens", "--by", "operation", "--json"])
        .current_dir(d.path())
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert!(v["data"]["aggregates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["key"] == "embed"));
}
