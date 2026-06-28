use std::fs;
use std::process::Command;

#[test]
fn release_package_contains_cli_mcp_skills_and_installer() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let temp = tempfile::tempdir().unwrap();
    let fake_bin = temp.path().join("kg");
    fs::write(&fake_bin, "#!/usr/bin/env sh\necho kgx-test\n").unwrap();

    let out_dir = temp.path().join("dist");
    let status = Command::new("bash")
        .arg(root.join("scripts/package-release.sh"))
        .env("KGX_RELEASE_BIN", &fake_bin)
        .env("KGX_RELEASE_OUT_DIR", &out_dir)
        .env("KGX_RELEASE_VERSION", "v0.1.0-test")
        .env("KGX_RELEASE_TARGET", "test-target")
        .current_dir(&root)
        .status()
        .unwrap();
    assert!(status.success());

    let archive = out_dir.join("kgx-v0.1.0-test-test-target.zip");
    assert!(archive.exists(), "{} missing", archive.display());
    assert!(out_dir
        .join("kgx-v0.1.0-test-test-target.zip.sha256")
        .exists());

    let listing = Command::new("python3")
        .args([
            "-c",
            "import sys, zipfile; print('\\n'.join(zipfile.ZipFile(sys.argv[1]).namelist()))",
            archive.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(listing.status.success());
    let listing = String::from_utf8(listing.stdout).unwrap();

    for expected in [
        "kgx-v0.1.0-test-test-target/bin/kg",
        "kgx-v0.1.0-test-test-target/install.sh",
        "kgx-v0.1.0-test-test-target/skills/claude/.mcp.json",
        "kgx-v0.1.0-test-test-target/skills/codex/config.toml",
        "kgx-v0.1.0-test-test-target/skills/opencode/opencode.json",
        "kgx-v0.1.0-test-test-target/skills/hooks/verify-finished.sh",
        "kgx-v0.1.0-test-test-target/MANIFEST.txt",
    ] {
        assert!(listing.contains(expected), "archive missing {expected}");
    }
}
