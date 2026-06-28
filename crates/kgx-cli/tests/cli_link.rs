use assert_cmd::Command;
mod common;

#[test]
fn link_orphans_finds_exactly_one() {
    let d = common::copy_fixture();
    let out = Command::cargo_bin("kg")
        .unwrap()
        .args(["link", "--orphans", "--json"])
        .current_dir(d.path())
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    let orph = v["data"]["orphans"].as_array().unwrap();
    assert_eq!(orph.len(), 1);
    assert_eq!(orph[0].as_str().unwrap(), "01FACT05ORPHAN0000000000");
}
