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
    let raw_dir = d.path().join(".brain/raw");
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

#[test]
fn capture_from_directory_ingests_all_text_files() {
    let d = common::copy_fixture();
    // Stage a folder of source files alongside the vault.
    let src = d.path().join("src");
    std::fs::create_dir_all(src.join("sub")).unwrap();
    std::fs::write(src.join("alpha.md"), "# Alpha spec\nAlpha detail.\n").unwrap();
    std::fs::write(src.join("beta.txt"), "Beta is a service.\n").unwrap();
    std::fs::write(src.join("sub/gamma.md"), "# Gamma\nGamma detail.\n").unwrap();
    std::fs::write(src.join("ignore.json"), "{}\n").unwrap(); // wrong ext

    let sources_before = std::fs::read_dir(d.path().join(".brain/notes/sources"))
        .unwrap()
        .filter_map(|e| e.ok())
        .count();

    let out = Command::cargo_bin("kg")
        .unwrap()
        .args(["capture", "--from", "src", "--type", "doc", "--json"])
        .current_dir(d.path())
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["command"], "capture");
    assert_eq!(v["data"]["captured"].as_u64().unwrap(), 3, "3 text files");
    assert_eq!(v["data"]["skipped"].as_u64().unwrap(), 0);

    // Three raw notes + three new source pointer notes under .brain/.
    let raw_count = std::fs::read_dir(d.path().join(".brain/raw"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
        .count();
    assert!(raw_count >= 3, "expected >=3 raw notes, got {raw_count}");
    let sources_after = std::fs::read_dir(d.path().join(".brain/notes/sources"))
        .unwrap()
        .filter_map(|e| e.ok())
        .count();
    assert_eq!(
        sources_after - sources_before,
        3,
        "expected 3 new source pointer notes"
    );
}
