use assert_cmd::Command;

#[test]
fn docs_usecase_writes_html() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("research.html");
    Command::cargo_bin("kg")
        .unwrap()
        .args(["docs", "usecase", "research", "--out"])
        .arg(&out)
        .assert()
        .success();
    let html = std::fs::read_to_string(out).unwrap();
    assert!(html.contains("Research"));
    assert!(html.contains("kg capture"));
}
