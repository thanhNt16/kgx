use assert_cmd::Command;
mod common;

#[test]
fn dashboard_json_reports_status_and_token_series() {
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
        .args(["dashboard", "--json"])
        .current_dir(d.path())
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["data"]["nodes"], 17);
    assert!(v["data"]["tokens_by_day"].is_array());
}
