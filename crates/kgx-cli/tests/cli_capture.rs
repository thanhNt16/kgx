use assert_cmd::Command;
mod common;

#[test]
fn capture_from_stdin_creates_immutable_raw() {
    let d = common::copy_fixture();
    Command::cargo_bin("kg")
        .unwrap()
        .args(["capture", "--from", "-", "--type", "doc", "--json"])
        .write_stdin("Redis is used for caching.")
        .current_dir(d.path())
        .assert()
        .success();
    let raw_dir = d.path().join("raw");
    let created: Vec<_> = std::fs::read_dir(&raw_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
        .collect();
    assert!(created.iter().any(|e| {
        std::fs::read_to_string(e.path())
            .unwrap()
            .contains("Redis is used for caching.")
    }));
}
