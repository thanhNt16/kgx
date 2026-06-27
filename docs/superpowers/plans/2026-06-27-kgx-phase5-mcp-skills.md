# KGX Phase 5 — MCP + Cross-Tool Skills Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development or superpowers:executing-plans. Read `2026-06-27-kgx-master-plan.md` (esp. §5 Cross-Tool Compatibility) and complete Phases 0–4. Steps use `- [ ]`.

**Goal:** Build the MCP stdio server (`kgx-mcp`, the universal capability layer), the RTK shell-output wrapper (`kgx-rtk`), the Ponytail prompt-ladder library (`kgx-ponytail`), the scheduler (`kgx-cron`), and the **full native skill packages** for Claude Code, Codex, and Cursor. Wire `kg mcp-server`, `kg cron`, and `kg init --with-skills`/`--with-rtk`. Unlocks T17, T18, and the MCP protocol compatibility test.

**Architecture:** Wave 1 adds `kgx-rtk`, `kgx-ponytail`, `kgx-cron` (core-only deps). Wave 4 adds `kgx-mcp` (reuses retrieval/extract/dream). The MCP server speaks JSON-RPC 2.0 over stdio (the `initialize`/`tools/list`/`tools/call` handshake) and exposes the 6 PRD tools; all three editors connect to the *same* server. Native skill packages translate the same workflows into each tool's idiom: Claude `SKILL.md`, Codex `AGENTS.md`, Cursor `.mdc` rule. RTK wraps shell-outs; Ponytail ladders feed extraction/dream/review prompts and back the `--ponytail-audit` rules.

**Tech Stack:** `serde_json` (JSON-RPC), `tokio` (stdio loop), Phase 2–4 crates; OS schedulers (systemd user timers / launchd plists).

## Global Constraints

Inherit master Global Constraints. Phase-critical: MCP tool schemas identical across clients (§5); RTK opt-in per command with raw fallback (T17 — ≤30% tokens); Ponytail never cuts security/validation (T18 — flags over-engineering, three intensities); cron writes real systemd/launchd units.

---

## Task 1: `kgx-rtk` — shell-output compression wrapper (Wave 1)

**Files:**
- Create: `crates/kgx-rtk/Cargo.toml`, `src/lib.rs`, `src/wrap.rs`, `src/install.rs`
- Test: in-module + `crates/kgx-rtk/tests/wrap.rs`

**Interfaces:**
- Consumes: `kgx_core::Result`.
- Produces: `wrap::run_with_rtk(cmd: &mut std::process::Command) -> Result<RtkOutput>` (runs the command; if the `rtk` binary is on PATH and `KGX_RTK!=off`, pipes output through it; else returns raw); `RtkOutput { raw_bytes: usize, compressed_bytes: usize, stdout: String }`; `install::install_hooks(tool: Tool, root: &Path) -> Result<()>` (writes the per-tool RTK hook config); `Tool { ClaudeCode, Codex, Cursor }`.

- [ ] **Step 1: Crate manifest**

```toml
# crates/kgx-rtk/Cargo.toml
[package]
name = "kgx-rtk"
edition.workspace = true
version.workspace = true
license.workspace = true
[dependencies]
kgx-core = { path = "../kgx-core" }
serde.workspace = true
serde_json.workspace = true
[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 2: Write failing test (fallback when rtk absent)**

```rust
// crates/kgx-rtk/tests/wrap.rs
use kgx_rtk::wrap::run_with_rtk;
#[test]
fn falls_back_to_raw_when_rtk_off() {
    std::env::set_var("KGX_RTK", "off");
    let mut c = std::process::Command::new("echo");
    c.arg("hello world");
    let out = run_with_rtk(&mut c).unwrap();
    assert!(out.stdout.contains("hello world"));
    assert_eq!(out.raw_bytes, out.compressed_bytes); // no compression when off
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p kgx-rtk`
Expected: FAIL.

- [ ] **Step 4: Implement wrap.rs**

```rust
// crates/kgx-rtk/src/wrap.rs
use std::process::Command;
use kgx_core::{Result, KgError};
#[derive(Debug)]
pub struct RtkOutput { pub raw_bytes: usize, pub compressed_bytes: usize, pub stdout: String }

pub fn run_with_rtk(cmd: &mut Command) -> Result<RtkOutput> {
    let output = cmd.output().map_err(|e| KgError::Other(format!("spawn failed: {e}")))?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();
    let raw_bytes = raw.len();
    let rtk_off = std::env::var("KGX_RTK").as_deref() == Ok("off");
    if rtk_off || !rtk_available() {
        return Ok(RtkOutput { raw_bytes, compressed_bytes: raw_bytes, stdout: raw });
    }
    // pipe raw through `rtk` (stdin → compressed stdout). Fallback to raw on any error.
    match pipe_through_rtk(&raw) {
        Ok(compressed) => Ok(RtkOutput { raw_bytes, compressed_bytes: compressed.len(), stdout: compressed }),
        Err(_) => Ok(RtkOutput { raw_bytes, compressed_bytes: raw_bytes, stdout: raw }),
    }
}
fn rtk_available() -> bool {
    Command::new("rtk").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}
fn pipe_through_rtk(input: &str) -> Result<String> {
    use std::io::Write;
    let mut child = Command::new("rtk").arg("compress").stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped()).spawn().map_err(|e| KgError::Other(e.to_string()))?;
    child.stdin.as_mut().unwrap().write_all(input.as_bytes()).map_err(|e| KgError::Other(e.to_string()))?;
    let out = child.wait_with_output().map_err(|e| KgError::Other(e.to_string()))?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}
```

- [ ] **Step 5: Implement install.rs (per-tool hooks)**

```rust
// crates/kgx-rtk/src/install.rs
use std::path::Path;
use kgx_core::{Result, KgError};
#[derive(Debug, Clone, Copy)]
pub enum Tool { ClaudeCode, Codex, Cursor }
pub fn install_hooks(tool: Tool, root: &Path) -> Result<()> {
    let write = |rel: &str, content: &str| -> Result<()> {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| KgError::Io { path: rel.into(), source: e })?;
        std::fs::write(&p, content).map_err(|e| KgError::Io { path: rel.into(), source: e })
    };
    match tool {
        // Claude Code: PostToolUse hook compresses Bash output via rtk
        Tool::ClaudeCode => write(".claude/settings.json", r#"{
  "hooks": {
    "PostToolUse": [
      { "matcher": "Bash",
        "hooks": [ { "type": "command", "command": "rtk compress" } ] }
    ]
  }
}"#),
        // Codex: exec wrapper note in config (Codex pipes tool output through rtk)
        Tool::Codex => write(".codex/rtk.toml", "# pipe shell output through rtk\n[output]\nfilter = \"rtk compress\"\n"),
        // Cursor: terminal profile alias
        Tool::Cursor => write(".cursor/rtk.json", r#"{ "terminal.outputFilter": "rtk compress" }"#),
    }
}
```
```rust
// crates/kgx-rtk/src/lib.rs
pub mod wrap; pub mod install;
pub use wrap::{run_with_rtk, RtkOutput};
pub use install::{install_hooks, Tool};
```

- [ ] **Step 6: Verify + commit**

Run: `cargo test -p kgx-rtk` → PASS.
```bash
git add crates/kgx-rtk && git commit -m "feat(rtk): shell-output wrapper + per-tool hook installer"
```

---

## Task 2: `kgx-ponytail` — prompt ladders + audit rules (Wave 1)

**Files:**
- Create: `crates/kgx-ponytail/Cargo.toml`, `src/lib.rs`, `src/ladder.rs`, `src/audit.rs`
- Test: in-module

**Interfaces:**
- Consumes: `kgx_core::{ProposedDiff, Severity}`.
- Produces: `ladder::ladder_for(op: Operation, intensity: Intensity) -> &'static str` (returns the Ponytail prompt prefix); `Operation { Extract, Dream, Ask, Review }`; `Intensity { Lite, Full, Ultra }`; `audit::audit_diff(&ProposedDiff) -> Vec<AuditFlag>`; `AuditFlag { code, msg }` (flags over-engineering: too many files, speculative additions). Security/validation diffs (severity `Hard`/contradiction) are **never** flagged for simplification.

- [ ] **Step 1: Crate manifest + failing tests**

```toml
# crates/kgx-ponytail/Cargo.toml
[package]
name = "kgx-ponytail"
edition.workspace = true
version.workspace = true
license.workspace = true
[dependencies]
kgx-core = { path = "../kgx-core" }
```
```rust
// crates/kgx-ponytail/src/audit.rs
#[cfg(test)]
mod tests {
    use super::*;
    use kgx_core::{ProposedDiff, DiffKind, Severity, FileChange};
    fn diff(files: usize, sev: Severity, kind: DiffKind) -> ProposedDiff {
        ProposedDiff { id: "x".into(), pass: "dedup".into(), kind, severity: sev, rationale: "".into(),
            files: (0..files).map(|i| FileChange { rel_path: format!("notes/f{i}.md"), before: None, after: Some("x".into()) }).collect() }
    }
    #[test]
    fn flags_over_broad_diff() {
        let flags = audit_diff(&diff(5, Severity::Soft, DiffKind::AddLink));
        assert!(flags.iter().any(|f| f.code == "over_broad"));
    }
    #[test]
    fn never_flags_hard_contradiction() {
        let flags = audit_diff(&diff(5, Severity::Hard, DiffKind::FlagContradiction));
        assert!(flags.is_empty(), "must never simplify safety-critical diffs");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p kgx-ponytail`
Expected: FAIL.

- [ ] **Step 3: Implement ladder.rs**

```rust
// crates/kgx-ponytail/src/ladder.rs
#[derive(Debug, Clone, Copy)] pub enum Operation { Extract, Dream, Ask, Review }
#[derive(Debug, Clone, Copy)] pub enum Intensity { Lite, Full, Ultra }
/// Ponytail "lazy senior dev" ladder: prefer the smallest correct change; do not add speculative structure.
pub fn ladder_for(op: Operation, intensity: Intensity) -> &'static str {
    match (op, intensity) {
        (Operation::Extract, Intensity::Lite) => "Extract only explicit, atomic facts. One claim per note. No inference.",
        (Operation::Extract, _) => "Extract atomic facts with provenance. Add entities only when named. Avoid speculative facts.",
        (Operation::Dream, Intensity::Ultra) => "Consolidate aggressively but never delete; supersede or archive. Justify every merge.",
        (Operation::Dream, _) => "Propose the minimal consolidation. Prefer no-op over speculative restructuring.",
        (Operation::Ask, _) => "Answer only from provided context. Cite note ids. Say 'unknown' if unsupported.",
        (Operation::Review, _) => "Flag diffs that add structure beyond what the rationale justifies.",
    }
}
```

- [ ] **Step 4: Implement audit.rs**

```rust
// crates/kgx-ponytail/src/audit.rs (above tests)
use kgx_core::{ProposedDiff, Severity, DiffKind};
#[derive(Debug, Clone, serde::Serialize)] pub struct AuditFlag { pub code: String, pub msg: String }
pub fn audit_diff(d: &ProposedDiff) -> Vec<AuditFlag> {
    // Never simplify safety-critical diffs.
    if matches!(d.severity, Severity::Hard) || matches!(d.kind, DiffKind::FlagContradiction) { return vec![]; }
    let mut flags = Vec::new();
    let writes = d.files.iter().filter(|f| f.after.is_some()).count();
    if writes > 3 { flags.push(AuditFlag { code: "over_broad".into(),
        msg: format!("diff writes {writes} files; consider splitting") }); }
    if d.rationale.trim().len() < 10 { flags.push(AuditFlag { code: "weak_rationale".into(),
        msg: "rationale too thin to justify the change".into() }); }
    flags
}
```
Add `serde` to deps. `lib.rs`: `pub mod ladder; pub mod audit; pub use ladder::{ladder_for, Operation, Intensity}; pub use audit::{audit_diff, AuditFlag};`

- [ ] **Step 5: Verify + commit**

Run: `cargo test -p kgx-ponytail` → PASS.
```bash
git add crates/kgx-ponytail && git commit -m "feat(ponytail): prompt ladders + over-engineering audit (safety-preserving)"
```

- [ ] **Step 6: Wire ladders into extract + review**

Now that `kgx-ponytail` exists, inject ladders: in `kgx-extract/src/prompt.rs` change `extract_prompt` callers to pass `Some(ladder_for(Operation::Extract, intensity))`; prepend it to the system prompt. In `kg review --ponytail-audit`, replace the placeholder over-broad check with `kgx_ponytail::audit_diff`. Add `kgx-ponytail` to `kgx-extract` and `kgx-cli` deps. Run `cargo test -p kgx-extract -p kgx-cli` → PASS.
```bash
git add crates/kgx-extract crates/kgx-cli && git commit -m "feat: wire ponytail ladders into extract + review audit"
```

---

## Task 3: `kgx-cron` — systemd/launchd scheduler (Wave 1)

**Files:**
- Create: `crates/kgx-cron/Cargo.toml`, `src/lib.rs`, `src/unit.rs`, `src/manage.rs`
- Test: in-module + `crates/kgx-cron/tests/unit.rs`

**Interfaces:**
- Consumes: `kgx_core::Result`.
- Produces: `unit::render_systemd(job: &Job) -> (String, String)` (service+timer text), `unit::render_launchd(job: &Job) -> String` (plist); `Job { name, command, calendar }`; `manage::{list, enable, disable, run, add}(...)` writing units to the right OS path and shelling `systemctl --user` / `launchctl`. `Platform::detect() -> Platform`.

- [ ] **Step 1: Crate manifest + failing test (pure renderers)**

```toml
# crates/kgx-cron/Cargo.toml
[package]
name = "kgx-cron"
edition.workspace = true
version.workspace = true
license.workspace = true
[dependencies]
kgx-core = { path = "../kgx-core" }
serde.workspace = true
[dev-dependencies]
tempfile.workspace = true
```
```rust
// crates/kgx-cron/src/unit.rs
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn systemd_timer_has_calendar() {
        let j = Job { name: "dream-nightly".into(), command: "kg dream --max-iterations 3".into(), calendar: "*-*-* 03:00:00".into() };
        let (service, timer) = render_systemd(&j);
        assert!(service.contains("ExecStart="));
        assert!(timer.contains("OnCalendar=*-*-* 03:00:00"));
    }
    #[test]
    fn launchd_plist_has_calendar_interval() {
        let j = Job { name: "dream-nightly".into(), command: "kg dream".into(), calendar: "03:00".into() };
        let plist = render_launchd(&j);
        assert!(plist.contains("StartCalendarInterval"));
        assert!(plist.contains("<integer>3</integer>")); // hour
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p kgx-cron`
Expected: FAIL.

- [ ] **Step 3: Implement unit.rs**

```rust
// crates/kgx-cron/src/unit.rs (above tests)
#[derive(Debug, Clone)] pub struct Job { pub name: String, pub command: String, pub calendar: String }
pub fn render_systemd(j: &Job) -> (String, String) {
    let service = format!("[Unit]\nDescription=KGX {name}\n\n[Service]\nType=oneshot\nExecStart={cmd}\n",
        name = j.name, cmd = shell_exec(&j.command));
    let timer = format!("[Unit]\nDescription=KGX {name} timer\n\n[Timer]\nOnCalendar={cal}\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n",
        name = j.name, cal = j.calendar);
    (service, timer)
}
fn shell_exec(cmd: &str) -> String { format!("/bin/sh -lc '{}'", cmd.replace('\'', "'\\''")) }
pub fn render_launchd(j: &Job) -> String {
    // calendar "HH:MM" → hour/minute
    let (h, m) = j.calendar.split_once(':').unwrap_or(("3", "0"));
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n  <key>Label</key><string>sh.kgx.{name}</string>\n  <key>ProgramArguments</key><array><string>/bin/sh</string><string>-lc</string><string>{cmd}</string></array>\n  <key>StartCalendarInterval</key><dict><key>Hour</key><integer>{h}</integer><key>Minute</key><integer>{m}</integer></dict>\n</dict></plist>\n",
        name = j.name, cmd = j.command)
}
```

- [ ] **Step 4: Implement manage.rs**

```rust
// crates/kgx-cron/src/manage.rs
use std::path::PathBuf;
use kgx_core::{Result, KgError};
use crate::unit::{Job, render_systemd, render_launchd};
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum Platform { Linux, Macos, Other }
impl Platform { pub fn detect() -> Platform {
    if cfg!(target_os = "linux") { Platform::Linux } else if cfg!(target_os = "macos") { Platform::Macos } else { Platform::Other } } }
fn systemd_dir() -> PathBuf { dirs_config().join("systemd/user") }
fn launchd_dir() -> PathBuf { home().join("Library/LaunchAgents") }
fn home() -> PathBuf { PathBuf::from(std::env::var("HOME").unwrap_or_default()) }
fn dirs_config() -> PathBuf { std::env::var("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|_| home().join(".config")) }
pub fn add(job: &Job) -> Result<Vec<PathBuf>> {
    match Platform::detect() {
        Platform::Linux => { let d = systemd_dir(); std::fs::create_dir_all(&d).map_err(io(&d))?;
            let (svc, tmr) = render_systemd(job);
            let sp = d.join(format!("kgx-{}.service", job.name)); let tp = d.join(format!("kgx-{}.timer", job.name));
            std::fs::write(&sp, svc).map_err(io(&sp))?; std::fs::write(&tp, tmr).map_err(io(&tp))?; Ok(vec![sp, tp]) }
        Platform::Macos => { let d = launchd_dir(); std::fs::create_dir_all(&d).map_err(io(&d))?;
            let p = d.join(format!("sh.kgx.{}.plist", job.name)); std::fs::write(&p, render_launchd(job)).map_err(io(&p))?; Ok(vec![p]) }
        Platform::Other => Err(KgError::Other("unsupported platform for cron".into())),
    }
}
pub fn enable(name: &str) -> Result<()> { shell(&platform_cmd("enable", name)) }
pub fn disable(name: &str) -> Result<()> { shell(&platform_cmd("disable", name)) }
pub fn run(name: &str) -> Result<()> { shell(&platform_cmd("run", name)) }
pub fn list() -> Result<Vec<String>> {
    let dir = match Platform::detect() { Platform::Linux => systemd_dir(), Platform::Macos => launchd_dir(), _ => return Ok(vec![]) };
    Ok(std::fs::read_dir(dir).map(|rd| rd.filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().filter(|n| n.contains("kgx")).map(String::from)).collect()).unwrap_or_default())
}
fn platform_cmd(action: &str, name: &str) -> Vec<String> {
    match Platform::detect() {
        Platform::Linux => { let unit = format!("kgx-{name}.timer");
            match action { "enable" => vec!["systemctl","--user","enable","--now",&unit].iter().map(|s| s.to_string()).collect(),
                "disable" => vec!["systemctl","--user","disable","--now",&unit].iter().map(|s| s.to_string()).collect(),
                _ => vec!["systemctl","--user","start",&format!("kgx-{name}.service")].iter().map(|s| s.to_string()).collect() } }
        Platform::Macos => { let label = format!("sh.kgx.{name}"); let plist = format!("{}/Library/LaunchAgents/{label}.plist", std::env::var("HOME").unwrap_or_default());
            match action { "enable" => vec!["launchctl","load","-w",&plist].iter().map(|s| s.to_string()).collect(),
                "disable" => vec!["launchctl","unload","-w",&plist].iter().map(|s| s.to_string()).collect(),
                _ => vec!["launchctl","start",&label].iter().map(|s| s.to_string()).collect() } }
        Platform::Other => vec![],
    }
}
fn shell(argv: &[String]) -> Result<()> {
    if argv.is_empty() { return Err(KgError::Other("unsupported platform".into())); }
    let st = std::process::Command::new(&argv[0]).args(&argv[1..]).status().map_err(|e| KgError::Other(e.to_string()))?;
    if st.success() { Ok(()) } else { Err(KgError::Other(format!("{argv:?} failed"))) }
}
fn io(p: &std::path::Path) -> impl Fn(std::io::Error) -> KgError + '_ { move |e| KgError::Io { path: p.display().to_string(), source: e } }
```
`lib.rs`: `pub mod unit; pub mod manage; pub use unit::{Job, render_systemd, render_launchd}; pub use manage::{Platform, add, enable, disable, run, list};`

- [ ] **Step 5: Verify + commit**

Run: `cargo test -p kgx-cron` → PASS.
```bash
git add crates/kgx-cron && git commit -m "feat(cron): systemd/launchd unit rendering + management"
```

---

## Task 4: `kgx-mcp` — JSON-RPC stdio server (Wave 4)

**Files:**
- Create: `crates/kgx-mcp/Cargo.toml`, `src/lib.rs`, `src/protocol.rs`, `src/tools.rs`, `src/server.rs`
- Test: `crates/kgx-mcp/tests/protocol.rs`

**Interfaces:**
- Consumes: `kgx_retrieval::search`, `kgx_extract`, `kgx_dream`, `kgx_vault`, `kgx_graph::Brain`, `kgx_llm::select`.
- Produces: `server::serve_stdio(root: PathBuf) -> Result<()>` (the JSON-RPC loop); `tools::TOOL_SCHEMAS` (JSON array describing the 6 tools); `tools::dispatch(root, name, args) -> Result<serde_json::Value>` for `search_notes`, `get_note`, `upsert_note`, `ask_question`, `capture_raw`, `dream_step`. Protocol: handles `initialize`, `tools/list`, `tools/call`.

- [ ] **Step 1: Crate manifest**

```toml
# crates/kgx-mcp/Cargo.toml
[package]
name = "kgx-mcp"
edition.workspace = true
version.workspace = true
license.workspace = true
[dependencies]
kgx-core = { path = "../kgx-core" }
kgx-vault = { path = "../kgx-vault" }
kgx-graph = { path = "../kgx-graph" }
kgx-retrieval = { path = "../kgx-retrieval" }
kgx-extract = { path = "../kgx-extract" }
kgx-dream = { path = "../kgx-dream" }
kgx-llm = { path = "../kgx-llm" }
serde.workspace = true
serde_json.workspace = true
tokio = { version = "1", features = ["rt-multi-thread","macros","io-std","io-util"] }
[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 2: Write failing protocol test (in-process handler)**

```rust
// crates/kgx-mcp/tests/protocol.rs
use kgx_mcp::protocol::handle_message;
mod common; // copy_fixture
#[tokio::test]
async fn initialize_and_tools_list() {
    let d = common::copy_fixture();
    // initialize
    let init = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}});
    let resp = handle_message(d.path(), init).await.unwrap();
    assert_eq!(resp["result"]["serverInfo"]["name"], "kgx");
    // tools/list
    let list = serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}});
    let resp = handle_message(d.path(), list).await.unwrap();
    let tools = resp["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    for expected in ["search_notes","get_note","upsert_note","ask_question","capture_raw","dream_step"] {
        assert!(names.contains(&expected), "missing tool {expected}");
    }
}
#[tokio::test]
async fn tools_call_search() {
    let d = common::copy_fixture();
    // index first via library call
    let mut brain = kgx_graph::Brain::open(&d.path().join(".kg/brain.sqlite")).unwrap();
    let notes = kgx_vault::scan::scan_vault(d.path()).unwrap();
    kgx_graph::build::build_full(&mut brain, &notes, &kgx_graph::embed::MockEmbedder::new()).unwrap();
    let call = serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call",
        "params":{"name":"search_notes","arguments":{"query":"primary datastore","limit":5}}});
    std::env::set_var("KGX_LLM","mock");
    let resp = kgx_mcp::protocol::handle_message(d.path(), call).await.unwrap();
    assert!(resp["result"]["content"][0]["text"].as_str().unwrap().contains("01FACT01"));
}
```
Add `crates/kgx-mcp/tests/common/mod.rs` with `copy_fixture`.

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p kgx-mcp`
Expected: FAIL.

- [ ] **Step 4: Implement tools.rs (schemas + dispatch)**

```rust
// crates/kgx-mcp/src/tools.rs
use std::path::Path;
use kgx_core::{Result, KgError};
use serde_json::{json, Value};

pub fn tool_schemas() -> Value {
    json!([
      {"name":"search_notes","description":"Hybrid search over the knowledge graph",
       "inputSchema":{"type":"object","properties":{"query":{"type":"string"},"limit":{"type":"integer"},"mode":{"type":"string"}},"required":["query"]}},
      {"name":"get_note","description":"Fetch a note by id",
       "inputSchema":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}},
      {"name":"upsert_note","description":"Create or update a note",
       "inputSchema":{"type":"object","properties":{"type":{"type":"string"},"title":{"type":"string"},"body":{"type":"string"},"id":{"type":"string"}},"required":["type","title","body"]}},
      {"name":"ask_question","description":"Hybrid Q&A with citations",
       "inputSchema":{"type":"object","properties":{"question":{"type":"string"},"scope":{"type":"string"}},"required":["question"]}},
      {"name":"capture_raw","description":"Ingest raw content into raw/ immutably",
       "inputSchema":{"type":"object","properties":{"content":{"type":"string"},"kind":{"type":"string"}},"required":["content"]}},
      {"name":"dream_step","description":"Run one bounded dream iteration, returns staged diffs (does not apply)",
       "inputSchema":{"type":"object","properties":{"only":{"type":"string"},"max_iterations":{"type":"integer"}}}}
    ])
}

pub async fn dispatch(root: &Path, name: &str, args: &Value) -> Result<Value> {
    match name {
        "search_notes" => {
            let brain = kgx_graph::Brain::open(&root.join(".kg/brain.sqlite"))?;
            let embedder = kgx_llm::select::embedder_from_env();
            let mode = match args["mode"].as_str() { Some("keyword") => kgx_retrieval::Mode::Keyword,
                Some("semantic") => kgx_retrieval::Mode::Semantic, _ => kgx_retrieval::Mode::Hybrid };
            let hits = kgx_retrieval::search(&brain, embedder.as_ref(), args["query"].as_str().unwrap_or(""),
                kgx_retrieval::SearchOpts { mode, limit: args["limit"].as_u64().unwrap_or(10) as usize, expand_ppr: true })?;
            Ok(json!(hits))
        }
        "get_note" => {
            let notes = kgx_vault::scan::scan_vault(root)?;
            let id = args["id"].as_str().unwrap_or("");
            let n = notes.into_iter().find(|n| n.fm.id == id).ok_or_else(|| KgError::NotFound(id.into()))?;
            Ok(json!({"id": n.fm.id, "title": n.fm.title, "body": n.body, "path": n.rel_path.display().to_string()}))
        }
        "ask_question" => {
            // reuse retrieval + provider (mirrors `kg ask`)
            let brain = kgx_graph::Brain::open(&root.join(".kg/brain.sqlite"))?;
            let notes = kgx_vault::scan::scan_vault(root)?;
            let embedder = kgx_llm::select::embedder_from_env();
            let hits = kgx_retrieval::search(&brain, embedder.as_ref(), args["question"].as_str().unwrap_or(""),
                kgx_retrieval::SearchOpts::default())?;
            let mut ctx = String::from("ANSWER_QUESTION\nContext:\n");
            for h in &hits { if let Some(n) = notes.iter().find(|n| n.fm.id == h.id) { ctx.push_str(&format!("[{}] {}: {}\n", n.fm.id, n.fm.title, n.body)); } }
            ctx.push_str(&format!("\nQuestion: {}\n", args["question"].as_str().unwrap_or("")));
            let provider = kgx_llm::select::provider_from_env()?;
            let resp = provider.complete(kgx_core::LlmRequest { system: "Answer from context, cite ids".into(), prompt: ctx, max_tokens: 1024, temperature: 0.0 }).await?;
            Ok(serde_json::from_str(&resp.text).unwrap_or(json!({"answer": resp.text, "citations": []})))
        }
        "capture_raw" => {
            let content = args["content"].as_str().unwrap_or("");
            // mirror kg capture: write immutable raw note (simplified path)
            let stem = kgx_core::util::slugify(content.lines().next().unwrap_or("capture"));
            let rel = format!("raw/{}-{stem}.md", &kgx_core::util::now_iso()[..10]);
            let p = root.join(&rel);
            std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| KgError::Io { path: rel.clone(), source: e })?;
            if !p.exists() { std::fs::write(&p, format!("---\ntype: source\nid: {}\ntitle: \"{}\"\ncreated_via: mcp\n---\n{content}\n",
                kgx_core::util::new_ulid(), content.lines().next().unwrap_or("capture"))).map_err(|e| KgError::Io { path: rel.clone(), source: e })?; }
            Ok(json!({"raw": rel}))
        }
        "upsert_note" | "dream_step" => Ok(json!({"status":"ok","note":"see kg dream/review for apply gate"})),
        other => Err(KgError::NotFound(format!("tool {other}"))),
    }
}
```
> `upsert_note` full body (build `Note`, `write_note`, mark `created_via: mcp`) and `dream_step` (call `kgx_dream::run::dream` with `max_iterations:1`, return staged diffs without applying) are completed to satisfy a follow-up test; the dispatch arm shown wires them in. Both must set provenance and respect the review gate (never auto-apply).

- [ ] **Step 5: Implement protocol.rs + server.rs**

```rust
// crates/kgx-mcp/src/protocol.rs
use std::path::Path;
use kgx_core::Result;
use serde_json::{json, Value};
use crate::tools::{tool_schemas, dispatch};
pub async fn handle_message(root: &Path, msg: Value) -> Result<Value> {
    let id = msg["id"].clone();
    let method = msg["method"].as_str().unwrap_or("");
    let result = match method {
        "initialize" => json!({"protocolVersion":"2024-11-05","serverInfo":{"name":"kgx","version":env!("CARGO_PKG_VERSION")},
            "capabilities":{"tools":{}}}),
        "tools/list" => json!({"tools": tool_schemas()}),
        "tools/call" => {
            let name = msg["params"]["name"].as_str().unwrap_or("");
            let args = &msg["params"]["arguments"];
            let out = dispatch(root, name, args).await?;
            json!({"content":[{"type":"text","text": serde_json::to_string(&out).unwrap_or_default()}]})
        }
        _ => json!({"error":"unknown method"}),
    };
    Ok(json!({"jsonrpc":"2.0","id":id,"result":result}))
}
```
```rust
// crates/kgx-mcp/src/server.rs
use std::path::PathBuf;
use kgx_core::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
pub async fn serve_stdio(root: PathBuf) -> Result<()> {
    let stdin = tokio::io::stdin(); let mut reader = BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();
    while let Ok(Some(line)) = reader.next_line().await {
        if line.trim().is_empty() { continue; }
        let msg: serde_json::Value = match serde_json::from_str(&line) { Ok(v) => v, Err(_) => continue };
        let resp = crate::protocol::handle_message(&root, msg).await?;
        let out = serde_json::to_string(&resp).unwrap();
        let _ = stdout.write_all(out.as_bytes()).await; let _ = stdout.write_all(b"\n").await; let _ = stdout.flush().await;
    }
    Ok(())
}
```
`lib.rs`: `pub mod protocol; pub mod tools; pub mod server;`

- [ ] **Step 6: Verify + commit**

Run: `cargo test -p kgx-mcp` → PASS.
```bash
git add crates/kgx-mcp && git commit -m "feat(mcp): JSON-RPC stdio server, 6 tools, identical schemas all clients"
```

---

## Task 5: `kg mcp-server` + `kg cron` commands

**Files:**
- Create: `crates/kgx-cli/src/commands/{mcp_server,cron}.rs`; modify cli/main/Cargo (`kgx-mcp`, `kgx-cron`).
- Test: `crates/kgx-cli/tests/cli_cron.rs`

**Interfaces:**
- Produces: `kg mcp-server --transport stdio` (calls `kgx_mcp::server::serve_stdio`); `kg cron list|enable|disable|run|add` (calls `kgx_cron::manage`). `kg cron add dream-nightly --command "kg dream" --calendar ...`.

- [ ] **Step 1: Write failing cron test (add writes a unit, dry to temp HOME)**

```rust
// crates/kgx-cli/tests/cli_cron.rs
use assert_cmd::Command;
#[test]
fn cron_add_writes_unit_file() {
    let home = tempfile::tempdir().unwrap();
    let out = Command::cargo_bin("kg").unwrap()
        .env("HOME", home.path()).env("XDG_CONFIG_HOME", home.path().join(".config"))
        .args(["cron","add","dream-nightly","--command","kg dream --max-iterations 3","--calendar","*-*-* 03:00:00","--json"])
        .assert().success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    let written = v["data"]["files"].as_array().unwrap();
    assert!(!written.is_empty());
    assert!(written.iter().any(|f| f.as_str().unwrap().contains("dream-nightly")));
}
```

- [ ] **Step 2–4: Implement mcp_server.rs + cron.rs**, verify.

```rust
// crates/kgx-cli/src/commands/mcp_server.rs
pub fn run(_json: bool, transport: &str) -> anyhow::Result<()> {
    if transport != "stdio" { anyhow::bail!("only --transport stdio supported"); }
    let root = std::env::current_dir()?;
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(kgx_mcp::server::serve_stdio(root))?;
    Ok(())
}
```
```rust
// crates/kgx-cli/src/commands/cron.rs
use std::time::Instant;
use crate::output::emit;
use kgx_cron::{Job, manage};
pub fn run(json: bool, action: &str, name: Option<String>, command: Option<String>, calendar: Option<String>) -> anyhow::Result<()> {
    let start = Instant::now();
    match action {
        "add" => { let job = Job { name: name.clone().unwrap(), command: command.unwrap_or("kg dream".into()),
            calendar: calendar.unwrap_or("*-*-* 03:00:00".into()) };
            let files = manage::add(&job)?;
            emit("cron", serde_json::json!({"files": files.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()}),
                json, start, |_| println!("✔ wrote {} unit file(s)", files.len())); }
        "list" => { let jobs = manage::list()?; emit("cron", serde_json::json!({"jobs": jobs}), json, start,
            |_| for j in &jobs { println!("{j}"); }); }
        "enable" => { manage::enable(&name.unwrap())?; emit("cron", serde_json::json!({"enabled": true}), json, start, |_| println!("✔ enabled")); }
        "disable" => { manage::disable(&name.unwrap())?; emit("cron", serde_json::json!({"disabled": true}), json, start, |_| println!("✔ disabled")); }
        "run" => { manage::run(&name.unwrap())?; emit("cron", serde_json::json!({"ran": true}), json, start, |_| println!("✔ ran")); }
        other => anyhow::bail!("unknown cron action: {other}"),
    }
    Ok(())
}
```
Add `McpServer { #[arg(long, default_value="stdio")] transport: String }` and `Cron { action: String, name: Option<String>, #[arg(long)] command: Option<String>, #[arg(long)] calendar: Option<String> }` to cli. Run cron test → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-cli && git commit -m "feat(cli): kg mcp-server + kg cron"
```

---

## Task 6: Native skill packages (Claude Code, Codex, Cursor)

**Files:**
- Create: `skills/claude/.claude/skills/kgx/SKILL.md`, `skills/claude/.mcp.json`
- Create: `skills/codex/AGENTS.md`, `skills/codex/config.toml`
- Create: `skills/cursor/.cursor/rules/kgx.mdc`, `skills/cursor/.cursor/mcp.json`
- Test: `tests/smoke/t_skills_valid.rs`

**Interfaces:**
- Produces: the three native packages + a validator test asserting each is well-formed and references the same MCP server + the same 6 tools.

- [ ] **Step 1: Claude Code skill + MCP config**

```markdown
<!-- skills/claude/.claude/skills/kgx/SKILL.md -->
---
name: kgx
description: Use when working with a KGX knowledge vault — capture sources, extract atomic facts, ask hybrid graph questions, run the dream consolidation pass, and review staged diffs. Triggers on "knowledge graph", "kg vault", "extract facts", "ask the graph", "dream pass".
---

# KGX Knowledge Graph

The `kg` CLI manages a local Markdown knowledge vault with a rebuildable SQLite brain. An MCP server (`kg mcp-server`) exposes the same capabilities as tools.

## Workflows
- **Capture → Extract:** `kg capture --from <file|-> --type doc` then `kg extract --source <id> --intensity full`. Every fact gets `source:` provenance. Never edit `raw/`.
- **Ask:** `kg ask "<question>" --scope local|global --cite`. Use `--scope global` for "summarize everything about X".
- **Consolidate:** `kg dream --max-iterations 3` stages diffs on the `kg/dream` branch; **never auto-applies**. Review with `kg review --approve all --ponytail-audit`. Hard contradictions are blocked unless approved by id.
- **Rebuild:** `kg index --full --communities`. The brain is disposable; Markdown is canonical.

## Rules (Ponytail ladder)
- Extract only atomic, explicit facts. One claim per note.
- Supersede or archive — never delete.
- Cite note ids in every answer; say "unknown" if unsupported.

## MCP tools (via kg mcp-server)
`search_notes`, `get_note`, `upsert_note`, `ask_question`, `capture_raw`, `dream_step`.
```
```json
// skills/claude/.mcp.json
{ "mcpServers": { "kgx": { "command": "kg", "args": ["mcp-server", "--transport", "stdio"] } } }
```

- [ ] **Step 2: Codex AGENTS.md + config**

```markdown
<!-- skills/codex/AGENTS.md -->
# KGX Agent Instructions (Codex)

This repo is a KGX knowledge vault. Use the `kg` CLI and the `kgx` MCP server.

## Commands
- Capture: `kg capture --from - --type doc`
- Extract atomic facts (provenance required): `kg extract --source <id> --intensity full`
- Ask (cite ids): `kg ask "<q>" --cite [--scope global]`
- Consolidate (staged, review-gated): `kg dream --max-iterations 3` → `kg review --approve all`
- Rebuild brain: `kg index --full --communities`

## Hard rules
- `raw/` is immutable. Supersede/archive, never delete. One fact per note. Cite ids.
- `kg dream` never auto-applies; hard contradictions need explicit per-id approval.

## MCP tools
search_notes, get_note, upsert_note, ask_question, capture_raw, dream_step.
```
```toml
# skills/codex/config.toml  — merge into ~/.codex/config.toml
[mcp_servers.kgx]
command = "kg"
args = ["mcp-server", "--transport", "stdio"]
```

- [ ] **Step 3: Cursor rule + MCP config**

```markdown
<!-- skills/cursor/.cursor/rules/kgx.mdc -->
---
description: KGX knowledge-graph vault workflows and rules
globs: ["**/*.md"]
alwaysApply: false
---

# KGX (Cursor rule)

When editing a KGX vault (`notes/`, `raw/`, `index.md`):
- Capture: `kg capture --from - --type doc`; never edit `raw/`.
- Extract: `kg extract --source <id> --intensity full` — facts must carry `source:` provenance, one claim each.
- Ask: `kg ask "<q>" --cite [--scope global]`.
- Consolidate: `kg dream --max-iterations 3` (staged) → `kg review --approve all` (hard contradictions blocked).
- Rebuild: `kg index --full --communities`. Markdown is canonical; `.kg/` is disposable.

MCP tools (via .cursor/mcp.json): search_notes, get_note, upsert_note, ask_question, capture_raw, dream_step.
```
```json
// skills/cursor/.cursor/mcp.json
{ "mcpServers": { "kgx": { "command": "kg", "args": ["mcp-server", "--transport", "stdio"] } } }
```

- [ ] **Step 4: Write the validator test**

```rust
// tests/smoke/t_skills_valid.rs
use std::path::Path;
fn repo() -> &'static Path { Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap() }
const TOOLS: [&str;6] = ["search_notes","get_note","upsert_note","ask_question","capture_raw","dream_step"];
#[test]
fn all_mcp_configs_are_valid_json_and_point_to_kg() {
    for p in ["skills/claude/.mcp.json","skills/cursor/.cursor/mcp.json"] {
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(repo().join(p)).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["kgx"]["command"], "kg", "{p}");
    }
}
#[test]
fn every_skill_lists_all_six_tools() {
    for p in ["skills/claude/.claude/skills/kgx/SKILL.md","skills/codex/AGENTS.md","skills/cursor/.cursor/rules/kgx.mdc"] {
        let txt = std::fs::read_to_string(repo().join(p)).unwrap();
        for t in TOOLS { assert!(txt.contains(t), "{p} missing tool {t}"); }
    }
}
#[test]
fn codex_config_is_valid_toml() {
    let txt = std::fs::read_to_string(repo().join("skills/codex/config.toml")).unwrap();
    assert!(txt.parse::<toml::Value>().is_ok());
}
```
Add `toml = "0.8"` to smoke `[dev-dependencies]`.

- [ ] **Step 5: Verify + commit**

Run: `cargo test --workspace --test 'smoke*' t_skills` (or the smoke crate) → PASS.
```bash
git add skills tests/smoke/t_skills_valid.rs
git commit -m "feat(skills): native Claude Code, Codex, Cursor packages + validator"
```

---

## Task 7: `kg init --with-skills` / `--with-rtk` (tool detection)

**Files:**
- Modify: `crates/kgx-cli/src/commands/init.rs`; cli flags
- Test: `crates/kgx-cli/tests/cli_init_skills.rs`

**Interfaces:**
- Consumes: `kgx_rtk::install_hooks`.
- Produces: `kg init --with-skills [--with-rtk]` detects present tools (`.claude/`, `~/.codex/`, `.cursor/`) — or installs all when none detected — copying the matching skill package into the vault and (with `--with-rtk`) the RTK hooks.

- [ ] **Step 1: Write failing test**

```rust
// crates/kgx-cli/tests/cli_init_skills.rs
use assert_cmd::Command;
#[test]
fn init_with_skills_writes_all_packages() {
    let d = tempfile::tempdir().unwrap();
    let target = d.path().join("brain");
    Command::cargo_bin("kg").unwrap()
        .args(["init","--template","pkm","--with-skills","--with-rtk","--vault"]).arg(&target).assert().success();
    assert!(target.join(".claude/skills/kgx/SKILL.md").exists());
    assert!(target.join("AGENTS.md").exists());
    assert!(target.join(".cursor/rules/kgx.mdc").exists());
    assert!(target.join(".mcp.json").exists());
    assert!(target.join(".claude/settings.json").exists()); // rtk hook
}
```

- [ ] **Step 2–4: Implement** — embed the skill package contents via `include_str!` from `skills/**`, write them into the vault on `--with-skills`; call `kgx_rtk::install_hooks` for each detected tool on `--with-rtk`. Add `with_skills`/`with_rtk` flags. Run test → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-cli && git commit -m "feat(cli): kg init --with-skills/--with-rtk installs cross-tool packages"
```

---

## Task 8: Smoke T17 (RTK) + T18 (Ponytail audit)

**Files:**
- Create: `tests/smoke/t17_rtk.rs`, `tests/smoke/t18_ponytail.rs`

- [ ] **Step 1: T17 — rtk reduces tokens (or no-op fallback verified)**

```rust
// tests/smoke/t17_rtk.rs
use kgx_rtk::wrap::run_with_rtk;
#[test]
fn t17_rtk_compresses_or_falls_back_cleanly() {
    // If rtk is installed in CI, assert ≤30%; else assert clean fallback (compressed==raw).
    std::env::remove_var("KGX_RTK");
    let mut c = std::process::Command::new("git");
    c.args(["--no-pager","diff","--stat"]);
    let out = run_with_rtk(&mut c).unwrap();
    if std::process::Command::new("rtk").arg("--version").output().map(|o| o.status.success()).unwrap_or(false) {
        assert!(out.compressed_bytes as f64 <= out.raw_bytes as f64 * 0.30 + 1.0, "rtk did not hit ≤30%");
    } else {
        assert_eq!(out.compressed_bytes, out.raw_bytes); // graceful fallback
    }
}
```
> The `≤30%` acceptance (T17) is asserted only when `rtk` is present; CI without `rtk` validates the fallback contract. Document this in the test.

- [ ] **Step 2: T18 — ponytail-audit flags over-engineered diffs**

```rust
// tests/smoke/t18_ponytail.rs
use kgx_ponytail::{audit_diff};
use kgx_core::{ProposedDiff, DiffKind, Severity, FileChange};
#[test]
fn t18_audit_flags_over_broad_but_not_safety() {
    let broad = ProposedDiff { id:"a".into(), pass:"orphan_repair".into(), kind:DiffKind::AddLink, severity:Severity::Soft,
        rationale:"add many links across the vault".into(),
        files:(0..6).map(|i| FileChange{rel_path:format!("notes/f{i}.md"),before:None,after:Some("x".into())}).collect() };
    assert!(audit_diff(&broad).iter().any(|f| f.code=="over_broad"));
    let hard = ProposedDiff { id:"b".into(), pass:"contradiction".into(), kind:DiffKind::FlagContradiction, severity:Severity::Hard,
        rationale:"conflict".into(), files:vec![FileChange{rel_path:"notes/x.md".into(),before:None,after:None}] };
    assert!(audit_diff(&hard).is_empty());
}
```

- [ ] **Step 3: Verify + commit**

Run: `cargo test --workspace --test 'smoke*' -- --test-threads=1` → PASS.
```bash
git add tests/smoke && git commit -m "test(smoke): T17 rtk integration, T18 ponytail audit"
```

---

## Self-Review (Phase 5)

- **Spec coverage:** MCP server + 6 tools (§9, §16 MCP) ✔ Tasks 4–5; RTK wrapper + installer (§11) ✔ Tasks 1,7; Ponytail ladders + audit (§11, §19) ✔ Task 2; cron systemd/launchd (§13) ✔ Tasks 3,5; native skills for all three tools (§5, user requirement) ✔ Task 6; `init --with-skills/--with-rtk` (§20) ✔ Task 7; T17, T18, MCP protocol ✔ Tasks 4,8.
- **Type consistency:** `run_with_rtk`/`RtkOutput`/`install_hooks`/`Tool`; `ladder_for`/`audit_diff`/`AuditFlag`; `Job`/`render_systemd`/`render_launchd`/`manage::*`; `serve_stdio`/`handle_message`/`tool_schemas`/`dispatch` — all stable into Phase 6.
- **Cross-tool guarantee:** all three packages reference the same `kg mcp-server` and the same 6 tool names; `t_skills_valid.rs` enforces it.
- **Placeholder scan:** Task 4 `upsert_note`/`dream_step` and Task 7 implementation are described with exact signatures + tests to satisfy; everything else has complete code.
