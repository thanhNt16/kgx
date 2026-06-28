use assert_cmd::Command;
mod common;

#[test]
fn ask_global_uses_community_summaries() {
    let d = common::copy_fixture();
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full", "--communities"])
        .current_dir(d.path())
        .assert()
        .success();
    let out = Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args([
            "ask",
            "Summarize the datastore knowledge",
            "--scope",
            "global",
            "--json",
        ])
        .current_dir(d.path())
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert!(v["ok"].as_bool().unwrap());
    assert!(!v["data"]["answer"].as_str().unwrap().is_empty());
}
