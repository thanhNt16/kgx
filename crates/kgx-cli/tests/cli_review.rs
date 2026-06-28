use assert_cmd::Command;
mod common;

#[test]
fn review_approve_all_applies_soft_but_blocks_hard() {
    let d = common::copy_fixture();
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full"])
        .current_dir(d.path())
        .assert()
        .success();
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["dream", "--max-iterations", "2"])
        .current_dir(d.path())
        .assert()
        .success();
    let out = Command::cargo_bin("kg")
        .unwrap()
        .args(["review", "--approve", "all", "--json"])
        .current_dir(d.path())
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert!(v["data"]["applied"].as_u64().unwrap() >= 1);
    assert!(v["data"]["blocked_hard"].as_u64().unwrap() >= 1);
    let pg = std::fs::read_to_string(d.path().join("notes/facts/f-postgres-primary.md")).unwrap();
    assert!(pg.contains("status: superseded"));
}
