use assert_cmd::Command;

#[test]
fn sync_status_runs_git_status_through_rtk_wrapper() {
    let d = tempfile::tempdir().unwrap();
    Command::new("git")
        .args(["init"])
        .current_dir(d.path())
        .assert()
        .success();
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_RTK", "off")
        .args(["sync", "status"])
        .current_dir(d.path())
        .assert()
        .success();
}
