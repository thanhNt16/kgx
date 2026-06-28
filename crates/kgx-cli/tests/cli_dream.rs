use assert_cmd::Command;
mod common;

#[test]
fn dream_stages_without_touching_files() {
    let d = common::copy_fixture();
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full"])
        .current_dir(d.path())
        .assert()
        .success();
    let path = d.path().join("notes/facts/f-postgres-primary.md");
    let before = std::fs::read_to_string(&path).unwrap();
    let out = Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["dream", "--max-iterations", "2", "--json"])
        .current_dir(d.path())
        .assert()
        .success();
    assert!(d.path().join(".kg/staged_diffs.json").exists());
    assert_eq!(before, std::fs::read_to_string(&path).unwrap());
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert!(v["data"]["staged"].as_u64().unwrap() >= 1);
    assert!(v["data"]["iterations"].as_u64().unwrap() <= 2);
}
