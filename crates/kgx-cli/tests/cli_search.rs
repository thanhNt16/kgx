use assert_cmd::Command;
mod common;

#[test]
fn search_hybrid_json_returns_hits() {
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
        .env("KGX_LLM", "mock")
        .args(["search", "primary datastore", "--mode", "hybrid", "--json"])
        .current_dir(d.path())
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert!(!v["data"]["hits"].as_array().unwrap().is_empty());
}
