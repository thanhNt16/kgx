#[test]
fn install_script_is_valid_bash() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let status = std::process::Command::new("bash")
        .args(["-n"])
        .arg(repo.join("install.sh"))
        .status()
        .unwrap();
    assert!(status.success(), "install.sh has syntax errors");
}
