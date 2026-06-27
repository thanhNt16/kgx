use assert_cmd::Command;
mod common;

#[test]
fn ask_returns_answer_with_citations() {
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
        .args([
            "ask",
            "What is the primary datastore?",
            "--cite",
            "--json",
        ])
        .current_dir(d.path())
        .assert()
        .success();
    let v: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert!(v["data"]["answer"]
        .as_str()
        .unwrap()
        .to_lowercase()
        .contains("postgres"));
    assert!(!v["data"]["citations"].as_array().unwrap().is_empty());
}
