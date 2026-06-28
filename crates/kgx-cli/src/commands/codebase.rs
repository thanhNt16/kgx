use std::time::Instant;

use crate::cli::CodebaseCommand;
use crate::output::emit;

const BIN: &str = "codebase-memory-mcp";
const REGEX_SPECIALS: &[char] = &[
    '\\', '.', '+', '*', '?', '(', ')', '|', '[', ']', '{', '}', '^', '$',
];

fn escape_regex_literal(query: &str) -> String {
    let mut out = String::with_capacity(query.len());
    for ch in query.chars() {
        if REGEX_SPECIALS.contains(&ch) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn search_name_pattern(query: &str) -> String {
    format!(".*{}.*", escape_regex_literal(query))
}

fn ensure_success(
    status: std::process::ExitStatus,
    action: &str,
    stderr: &str,
) -> anyhow::Result<()> {
    if status.success() {
        return Ok(());
    }
    let detail = stderr.trim();
    if detail.is_empty() {
        anyhow::bail!("{action} failed (exit code: {:?})", status.code());
    }
    anyhow::bail!("{action} failed (exit code: {:?}): {detail}", status.code());
}

fn find_binary() -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let candidate = dir.join(BIN);
            if candidate.is_file() {
                Some(candidate)
            } else {
                None
            }
        })
    })
}

fn require_binary() -> anyhow::Result<std::path::PathBuf> {
    find_binary().ok_or_else(|| {
        anyhow::anyhow!(
            "codebase-memory-mcp not found on PATH.\nRun `kg codebase install` first, or install manually from https://github.com/DeusData/codebase-memory-mcp"
        )
    })
}

fn cbm_cli(tool: &str, args: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
    let bin = require_binary()?;
    let args_str = serde_json::to_string(args)?;
    let output = std::process::Command::new(&bin)
        .arg("cli")
        .arg(tool)
        .arg(&args_str)
        .output()
        .map_err(|e| anyhow::anyhow!("failed to run codebase-memory-mcp: {e}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure_success(output.status, "codebase-memory-mcp", &stderr)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(serde_json::from_str(&stdout).unwrap_or(serde_json::json!({"raw": stdout})))
}

fn install_binary(json: bool) -> anyhow::Result<()> {
    let start = Instant::now();
    if find_binary().is_some() {
        emit("codebase-install", "already-installed", json, start, |_| {
            println!("codebase-memory-mcp already installed");
        });
        return Ok(());
    }
    eprint!("Downloading codebase-memory-mcp...");
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(
            "curl -fsSL https://raw.githubusercontent.com/DeusData/codebase-memory-mcp/main/install.sh | bash",
        )
        .status()
        .map_err(|e| anyhow::anyhow!("failed to run install script: {e}"))?;
    if !status.success() {
        anyhow::bail!("install script failed (exit code: {:?})", status.code());
    }
    let bin = find_binary();
    if bin.is_none() {
        anyhow::bail!("install script ran but codebase-memory-mcp binary not found on PATH");
    }
    emit("codebase-install", "installed", json, start, |_| {
        println!("installed: {}", bin.unwrap().display());
    });
    Ok(())
}

fn update_binary(json: bool) -> anyhow::Result<()> {
    let start = Instant::now();
    let bin = require_binary()?;
    let output = std::process::Command::new(&bin)
        .arg("update")
        .output()
        .map_err(|e| anyhow::anyhow!("failed to run update: {e}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure_success(output.status, "update", &stderr)?;
    let msg = String::from_utf8_lossy(&output.stdout).to_string();
    emit("codebase-update", &msg, json, start, |m| {
        println!("{}", m.trim());
    });
    Ok(())
}

fn run_index(path: Option<std::path::PathBuf>, json: bool) -> anyhow::Result<()> {
    let start = Instant::now();
    let repo_path = path.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let abs = std::fs::canonicalize(&repo_path)
        .map_err(|e| anyhow::anyhow!("invalid path {}: {e}", repo_path.display()))?;
    let result = cbm_cli(
        "index_repository",
        &serde_json::json!({"repo_path": abs.to_string_lossy()}),
    )?;
    emit("codebase-index", &result, json, start, |r| {
        let status = r["status"].as_str().unwrap_or("unknown");
        print!("indexed codebase: {status}");
        if let Some(nodes) = r["nodes"].as_u64() {
            print!(", {nodes} nodes");
        }
        if let Some(edges) = r["edges"].as_u64() {
            print!(", {edges} edges");
        }
        println!();
    });
    Ok(())
}

fn run_search(query: &str, limit: usize, json: bool) -> anyhow::Result<()> {
    let start = Instant::now();
    let result = cbm_cli(
        "search_graph",
        &serde_json::json!({"name_pattern": search_name_pattern(query), "limit": limit}),
    )?;
    emit("codebase-search", &result, json, start, |r| {
        let results = r["results"].as_array().map(|v| v.len()).unwrap_or(0);
        println!("found {results} results for '{query}':");
        if let Some(items) = r["results"].as_array() {
            for item in items.iter().take(limit) {
                let name = item["name"].as_str().unwrap_or("?");
                let label = item["label"].as_str().unwrap_or("?");
                let file = item["file"].as_str().unwrap_or("?");
                println!("  {label} {name}  ({file})");
            }
        }
        if results == 0 {
            println!("  (no matches — try `kg codebase index` first)");
        }
    });
    Ok(())
}

fn run_trace(function: &str, direction: &str, json: bool) -> anyhow::Result<()> {
    let start = Instant::now();
    let result = cbm_cli(
        "trace_path",
        &serde_json::json!({"function_name": function, "direction": direction}),
    )?;
    emit("codebase-trace", &result, json, start, |r| {
        let path = r["path"].as_array().map(|v| v.len()).unwrap_or(0);
        if path == 0 {
            println!("no call path found for '{function}'");
        } else {
            println!("call path ({direction}) for '{function}':");
            if let Some(edges) = r["path"].as_array() {
                for edge in edges {
                    let src = edge["source"].as_str().unwrap_or("?");
                    let dst = edge["target"].as_str().unwrap_or("?");
                    let rel = edge["relationship"].as_str().unwrap_or("calls");
                    println!("  {src} --[{rel}]--> {dst}");
                }
            }
        }
    });
    Ok(())
}

fn run_architecture(json: bool) -> anyhow::Result<()> {
    let start = Instant::now();
    let result = cbm_cli("get_architecture", &serde_json::json!({}))?;
    emit("codebase-architecture", &result, json, start, |r| {
        println!("Codebase Architecture:");
        if let Some(langs) = r["languages"].as_array() {
            print!("  Languages:");
            for lang in langs {
                print!(" {lang}");
            }
            println!();
        }
        if let Some(packages) = r["packages"].as_array() {
            println!("  Packages: {}", packages.len());
        }
        if let Some(routes) = r["routes"].as_array() {
            println!("  Routes:");
            for route in routes {
                let method = route["method"].as_str().unwrap_or("ANY");
                let path = route["path"].as_str().unwrap_or("/");
                println!("    {method} {path}");
            }
        }
        if let Some(entry_points) = r["entry_points"].as_array() {
            println!("  Entry points: {}", entry_points.len());
            for ep in entry_points {
                println!("    {ep}");
            }
        }
    });
    Ok(())
}

fn run_status(json: bool) -> anyhow::Result<()> {
    let start = Instant::now();
    let result = cbm_cli("list_projects", &serde_json::json!({}))?;
    emit("codebase-status", &result, json, start, |r| {
        let projects = r["projects"].as_array().map(|v| v.len()).unwrap_or(0);
        if projects == 0 {
            println!("no indexed projects (run `kg codebase index`)");
        } else if let Some(items) = r["projects"].as_array() {
            println!("indexed projects ({projects}):");
            for proj in items {
                let name = proj["name"].as_str().unwrap_or("?");
                let nodes = proj["nodes"].as_u64().unwrap_or(0);
                let edges = proj["edges"].as_u64().unwrap_or(0);
                println!("  {name}  ({nodes} nodes, {edges} edges)");
            }
        }
    });
    Ok(())
}

pub fn run(json: bool, command: CodebaseCommand) -> anyhow::Result<()> {
    match command {
        CodebaseCommand::Install => install_binary(json),
        CodebaseCommand::Update => update_binary(json),
        CodebaseCommand::Index { path } => run_index(path, json),
        CodebaseCommand::Search { query, limit } => run_search(&query, limit, json),
        CodebaseCommand::Trace {
            function,
            direction,
        } => run_trace(&function, &direction, json),
        CodebaseCommand::Architecture => run_architecture(json),
        CodebaseCommand::Status => run_status(json),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn regex_search_pattern_escapes_user_query() {
        assert_eq!(
            super::search_name_pattern("foo.bar[1]"),
            ".*foo\\.bar\\[1\\].*"
        );
    }

    #[test]
    fn failed_external_status_is_an_error() {
        let status = std::process::Command::new("false").status().unwrap();
        let err = super::ensure_success(status, "update", "").unwrap_err();
        assert!(err.to_string().contains("update failed"));
    }
}
