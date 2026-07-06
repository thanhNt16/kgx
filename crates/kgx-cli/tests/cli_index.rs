mod common;
use assert_cmd::Command;

#[test]
fn index_full_builds_brain() {
    let d = common::copy_fixture();
    let out = Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full", "--pagerank", "--json"])
        .current_dir(d.path())
        .assert()
        .success();
    assert!(d.path().join(".brain/.kg/brain.sqlite").exists());
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["command"], "index");
    assert!(
        v["data"]["nodes"].as_u64().unwrap() >= 15,
        "expected >= 15 nodes, got {}",
        v["data"]["nodes"]
    );
    assert!(d.path().join(".brain/.kg/metrics.log").exists());
}
