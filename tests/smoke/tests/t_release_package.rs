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

/// The inner installer must (a) actually launch the binary it installs and
/// (b) recover when re-installing over a stale target. Regression for the
/// "kg hangs forever after install" bug, where a downloaded adhoc-signed
/// binary could stall in dyld and the installer reported success anyway.
#[test]
fn inner_installer_launches_binary_and_survives_reinstall() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let temp = tempfile::tempdir().unwrap();

    // A fake binary that genuinely handles --version (exits 0). The hardened
    // installer validates `<dst> --version`, so the fake must honor it.
    let fake_bin = temp.path().join("kg");
    fs::write(
        &fake_bin,
        "#!/usr/bin/env sh\n[ \"$1\" = \"--version\" ] && echo kgx-test && exit 0\necho ran\n",
    )
    .unwrap();

    let out_dir = temp.path().join("dist");
    let pkg_status = Command::new("bash")
        .arg(root.join("scripts/package-release.sh"))
        .env("KGX_RELEASE_BIN", &fake_bin)
        .env("KGX_RELEASE_OUT_DIR", &out_dir)
        .env("KGX_RELEASE_VERSION", "v0.1.0-test")
        .env("KGX_RELEASE_TARGET", "test-target")
        .current_dir(&root)
        .status()
        .unwrap();
    assert!(pkg_status.success());

    // Extract the packaged archive and run its install.sh into an isolated
    // HOME so we don't touch the developer's real ~/.local/bin.
    let archive = out_dir.join("kgx-v0.1.0-test-test-target.zip");
    let extract_dir = temp.path().join("extract");
    fs::create_dir_all(&extract_dir).unwrap();
    let unzip_status = Command::new("python3")
        .args([
            "-c",
            "import sys, zipfile; zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])",
            archive.to_str().unwrap(),
            extract_dir.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(unzip_status.success());

    let fake_home = temp.path().join("home");
    let bin_dir = fake_home.join(".local/bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let inner_install = extract_dir.join("kgx-v0.1.0-test-test-target/install.sh");

    // Pre-create a stale target file (simulates re-install over an old binary).
    fs::write(bin_dir.join("kg"), "#!/usr/bin/env sh\nexit 1\n").unwrap();

    let install_status = Command::new("bash")
        .arg(&inner_install)
        .env("HOME", &fake_home)
        .env("KGX_BIN_DIR", &bin_dir)
        // Don't touch the developer's real agent MCP configs during the test.
        .env("KGX_SHARE_DIR", fake_home.join(".kgx"))
        .output()
        .unwrap();
    assert!(
        install_status.status.success(),
        "inner install.sh failed: stderr={}",
        String::from_utf8_lossy(&install_status.stderr)
    );

    // The installed binary must be responsive -- this is the core regression
    // assertion. Before the fix, the installer copied bytes that hung at dyld
    // startup and reported success.
    let installed = bin_dir.join("kg");
    assert!(installed.exists(), "installed binary missing");
    let probe = Command::new(&installed).arg("--version").output().unwrap();
    assert!(
        probe.status.success(),
        "installed kg --version failed: {}",
        String::from_utf8_lossy(&probe.stderr)
    );
    assert!(
        String::from_utf8_lossy(&probe.stdout).contains("kgx-test"),
        "installed kg --version produced unexpected output: {}",
        String::from_utf8_lossy(&probe.stdout)
    );
}
