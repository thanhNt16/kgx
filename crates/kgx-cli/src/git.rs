use std::path::Path;

/// Ensure the named git branch exists and is checked out.
/// Best-effort: if not in a git repo, returns Ok(()) silently.
pub fn ensure_branch(root: &Path, name: &str) -> anyhow::Result<()> {
    // Check if we're in a git repo at all
    let is_git = std::process::Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(root)
        .output()?
        .status
        .success();

    if !is_git {
        return Ok(());
    }

    // Check if the branch already exists
    let exists = std::process::Command::new("git")
        .args(["rev-parse", "--verify", name])
        .current_dir(root)
        .output()?
        .status
        .success();

    let args: Vec<&str> = if exists {
        vec!["checkout", name]
    } else {
        vec!["checkout", "-b", name]
    };

    let st = std::process::Command::new("git")
        .args(&args)
        .current_dir(root)
        .output()?;

    if !st.status.success() {
        // Non-fatal: log but don't fail the dream command
        let stderr = String::from_utf8_lossy(&st.stderr);
        eprintln!("git branch warning: {}", stderr.trim());
    }

    Ok(())
}
