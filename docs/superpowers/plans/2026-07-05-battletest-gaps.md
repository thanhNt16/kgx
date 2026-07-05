# Battle-Test Gaps Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the 8 battle-test gaps (semantic search default, POLE taxonomy, graph viz export, contradiction pairing, macOS cron + remove, `review --reject`, `kg refine`, harness parity) and add ZCode as a 5th native harness.

**Architecture:** Evolve the embedded SQLite brain in place (sqlite-vec + FTS5 + edges/petgraph). No new storage engine. The brain is disposable (rebuilt from Markdown by `kg index --full`), so schema changes are re-index events, not migrations of user data. Every retrieval change is gated by the `bench/` harness.

**Tech Stack:** Rust 1.78 (edition 2021), rusqlite (bundled) + sqlite-vec, fastembed 4 (ONNX), clap 4, serde/serde_yaml, tokio, Cytoscape.js (vendored asset), launchd/systemd unit generation.

**Spec:** `docs/superpowers/specs/2026-07-05-battletest-gaps-design.md`

## Global Constraints

- Rust 1.78, edition 2021. `cargo fmt --all` and `cargo clippy --workspace` must stay clean; `git diff --check` must pass (no trailing whitespace / blank line at EOF — enforced by the repo's finish hook).
- All existing tests stay green: `cargo test --workspace` (101+ tests).
- Never-delete semantics: `Status` is `Active | Deprecated | Archived | Superseded` — no deletion paths.
- All vault mutations go through staged diffs (`.kg/staged_diffs.json`) + `kg review`; `Severity::Hard` diffs are never auto-applied by `--approve all`.
- Fail loud: unsupported input → error listing supported forms; degraded mode (mock embedder) → visible warning + `kg status` indicator. Never silently emit wrong output.
- LLM cost bounds: contradiction candidate pairs capped via `KGX_DREAM_MAX_PAIRS` (default 200).
- Retrieval acceptance (WS1): on the `bench/` corpus, the 5 previously-zero-recall gold questions become hits; Recall@5 ≥ 0.85; MRR does not regress below 0.633. Run: `python3 bench/bench.py` (corpus via `python3 bench/gen_corpus.py`).
- MCP tool names source of truth: `crates/kgx-mcp/src/tools/mod.rs` (9 tools: `nl_query_memory, query_memory, deep_search_memory, get_note, ingest_conversation, ingest_file, ingest_url, upsert_note, dream_step`).
- Commit after every task with the trailer: `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.

---

### Task 1: WS1 — Default the embedder to fastembed (`semantic` on by default)

**Files:**
- Modify: `crates/kgx-graph/Cargo.toml` (features)
- Modify: `crates/kgx-llm/Cargo.toml` (features)
- Modify: `crates/kgx-cli/Cargo.toml` (features)
- Modify: `crates/kgx-llm/src/select.rs`
- Test: `crates/kgx-llm/src/select.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Consumes: `kgx_core::llm::Embedder` trait; `kgx_graph::embed::{MockEmbedder, FastEmbedEmbedder}`.
- Produces: `pub enum EmbedChoice { Off, Mock, MiniLm, FastEmbed }`, `pub fn embed_choice(var: Option<&str>, semantic_built: bool, candle_built: bool) -> EmbedChoice`, `pub fn embedder_label() -> String` — Task 2 (`kg status`) uses `embedder_label()`.

- [ ] **Step 1: Write the failing test** — append to `crates/kgx-llm/src/select.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embed_choice_defaults_to_fastembed_when_semantic_built() {
        assert_eq!(embed_choice(None, true, false), EmbedChoice::FastEmbed);
    }

    #[test]
    fn embed_choice_falls_back_to_mock_without_semantic_build() {
        assert_eq!(embed_choice(None, false, false), EmbedChoice::Mock);
    }

    #[test]
    fn embed_choice_off_and_mock_opt_out() {
        assert_eq!(embed_choice(Some("off"), true, false), EmbedChoice::Off);
        assert_eq!(embed_choice(Some("mock"), true, false), EmbedChoice::Mock);
    }

    #[test]
    fn embed_choice_explicit_backends() {
        assert_eq!(embed_choice(Some("fastembed"), true, false), EmbedChoice::FastEmbed);
        assert_eq!(embed_choice(Some("minilm"), true, true), EmbedChoice::MiniLm);
        // requesting a backend that isn't compiled in falls back to mock
        assert_eq!(embed_choice(Some("fastembed"), false, false), EmbedChoice::Mock);
        assert_eq!(embed_choice(Some("minilm"), true, false), EmbedChoice::Mock);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kgx-llm embed_choice -- --nocapture`
Expected: FAIL — `embed_choice` / `EmbedChoice` not found.

- [ ] **Step 3: Implement selection logic** — in `crates/kgx-llm/src/select.rs`, add above `embedder_from_env` and rewrite it:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbedChoice {
    Off,
    Mock,
    MiniLm,
    FastEmbed,
}

/// Pure selection logic so it is unit-testable. `var` is KGX_EMBED.
pub fn embed_choice(var: Option<&str>, semantic_built: bool, candle_built: bool) -> EmbedChoice {
    match var {
        Some("off") => EmbedChoice::Off,
        Some("mock") => EmbedChoice::Mock,
        Some("minilm") if candle_built => EmbedChoice::MiniLm,
        Some("minilm") => EmbedChoice::Mock,
        Some("fastembed") | None if semantic_built => EmbedChoice::FastEmbed,
        Some(_) | None => EmbedChoice::Mock,
    }
}

/// Human-readable label for `kg status` / warnings.
pub fn embedder_label() -> String {
    let var = std::env::var("KGX_EMBED").ok();
    match embed_choice(var.as_deref(), cfg!(feature = "semantic"), cfg!(feature = "candle")) {
        EmbedChoice::FastEmbed => "fastembed (semantic)".into(),
        EmbedChoice::MiniLm => "minilm (semantic)".into(),
        EmbedChoice::Off => "off (keyword-only, explicit)".into(),
        EmbedChoice::Mock => "mock (keyword-only — semantic search DISABLED)".into(),
    }
}

pub fn embedder_from_env() -> Box<dyn Embedder> {
    let var = std::env::var("KGX_EMBED").ok();
    let choice = embed_choice(var.as_deref(), cfg!(feature = "semantic"), cfg!(feature = "candle"));
    match choice {
        #[cfg(feature = "candle")]
        EmbedChoice::MiniLm => match kgx_graph::embed::MiniLmEmbedder::load() {
            Ok(e) => return Box::new(e),
            Err(e) => {
                eprintln!("warning: minilm failed to load, falling back to mock: {e}");
                return Box::new(kgx_graph::embed::MockEmbedder::new());
            }
        },
        #[cfg(feature = "semantic")]
        EmbedChoice::FastEmbed => match kgx_graph::embed::FastEmbedEmbedder::load() {
            Ok(e) => return Box::new(e),
            Err(e) => {
                eprintln!("warning: fastembed failed to load, falling back to mock (semantic search disabled): {e}");
                return Box::new(kgx_graph::embed::MockEmbedder::new());
            }
        },
        _ => {}
    }
    if !matches!(choice, EmbedChoice::Off | EmbedChoice::Mock) || (var.is_none() && !cfg!(feature = "semantic")) {
        eprintln!("warning: using mock embedder — semantic search disabled (build with the default `semantic` feature)");
    }
    Box::new(kgx_graph::embed::MockEmbedder::new())
}
```

Note: the old `embedder_from_env` body (lines 40–56) is fully replaced. The `#[cfg]` match arms require the enum variants to exist regardless of features — they do, since `EmbedChoice` is feature-independent.

- [ ] **Step 4: Flip default features.** In `crates/kgx-graph/Cargo.toml` change `default = []` → `default = ["semantic"]`. In `crates/kgx-llm/Cargo.toml` change `default = []` → `default = ["semantic"]`. In `crates/kgx-cli/Cargo.toml` change `default = []` → `default = ["semantic"]`.

- [ ] **Step 5: Run tests**

Run: `cargo test -p kgx-llm && cargo build -p kgx-cli`
Expected: PASS (first build compiles fastembed/ort — takes several minutes).

- [ ] **Step 6: Run the full workspace test suite**

Run: `cargo test --workspace`
Expected: PASS, no regressions. (Tests that construct embedders explicitly use `MockEmbedder` and are unaffected; `embedder_from_env` is only called by CLI paths.)

- [ ] **Step 7: Benchmark acceptance**

Run: `cargo build --release --bin kg && python3 bench/gen_corpus.py && python3 bench/bench.py`
Expected: the 5 previously-zero gold questions (Q2, Q4, Q6, Q13, Q14) now have Recall@5 > 0; overall Recall@5 ≥ 0.85; MRR ≥ 0.633. If the sandbox blocks the model download, note it and re-run where network is available before merging.

- [ ] **Step 8: Commit**

```bash
git add crates/kgx-graph/Cargo.toml crates/kgx-llm/Cargo.toml crates/kgx-cli/Cargo.toml crates/kgx-llm/src/select.rs
git commit -m "feat(ws1): default embedder to fastembed; KGX_EMBED=off opts out

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: WS1 — Surface the active embedder in `kg status`

**Files:**
- Modify: `crates/kgx-cli/src/commands/status.rs`

**Interfaces:**
- Consumes: `kgx_llm::select::embedder_label() -> String` (Task 1).
- Produces: `StatusSnapshot.embedder: String` field; `kg status` prints `embedder=<label>`.

- [ ] **Step 1: Write the failing test** — append to `crates/kgx-cli/src/commands/status.rs`:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn embedder_label_is_never_empty() {
        assert!(!kgx_llm::select::embedder_label().is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify current state**

Run: `cargo test -p kgx-cli embedder_label -- --nocapture`
Expected: PASS if Task 1 landed (this pins the dependency); FAIL to compile if Task 1 is missing — do Task 1 first.

- [ ] **Step 3: Add the field.** In `StatusSnapshot` add `pub embedder: String,`. In `snapshot()` set `embedder: kgx_llm::select::embedder_label(),` in the returned struct. In `run()` change the println closure to:

```rust
println!(
    "nodes={} edges={} orphans={} pending={} embedder={}",
    s.nodes, s.edges, s.orphans, s.pending_diffs, s.embedder
)
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p kgx-cli && cargo test --workspace`
Expected: PASS. If any smoke test asserts on the exact `kg status` line, update it to include `embedder=`.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-cli/src/commands/status.rs
git commit -m "feat(ws1): show active embedder in kg status

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: WS5 — Calendar parser + correct launchd rendering

**Files:**
- Create: `crates/kgx-cron/src/calendar.rs`
- Modify: `crates/kgx-cron/src/unit.rs`
- Modify: `crates/kgx-cron/src/lib.rs` (add `pub mod calendar;`)
- Modify: `crates/kgx-cron/src/manage.rs` (`add()` validates before writing)

**Interfaces:**
- Produces: `pub enum Schedule { Hourly { minute: u8 }, Daily { hour: u8, minute: u8 }, Weekly { weekday: u8, hour: u8, minute: u8 } }` (weekday: launchd numbering, 0=Sun..6=Sat); `pub fn parse_calendar(s: &str) -> Result<Schedule>`; `render_launchd(&Job) -> Result<String>` (was infallible).
- Consumes: `kgx_core::{KgError, Result}`.

- [ ] **Step 1: Write the failing tests** — create `crates/kgx-cron/src/calendar.rs`:

```rust
use kgx_core::{KgError, Result};

/// Parsed schedule, normalized to launchd-compatible fields.
/// weekday: 0=Sun, 1=Mon, ... 6=Sat (launchd numbering).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schedule {
    Hourly { minute: u8 },
    Daily { hour: u8, minute: u8 },
    Weekly { weekday: u8, hour: u8, minute: u8 },
}

const SUPPORTED: &str = "supported calendar forms: 'hourly', 'daily', 'weekly', 'HH:MM', '*-*-* HH:MM:SS', '<Mon..Sun> *-*-* HH:MM:SS'";

fn parse_hm(s: &str) -> Option<(u8, u8)> {
    let (h, m) = s.split_once(':')?;
    let h: u8 = h.trim().parse().ok()?;
    let m: u8 = m.trim().parse().ok()?;
    (h < 24 && m < 60).then_some((h, m))
}

fn parse_hms(s: &str) -> Option<(u8, u8)> {
    let mut it = s.splitn(3, ':');
    let h: u8 = it.next()?.trim().parse().ok()?;
    let m: u8 = it.next()?.trim().parse().ok()?;
    let sec: u8 = it.next().unwrap_or("0").trim().parse().ok()?;
    (h < 24 && m < 60 && sec < 60).then_some((h, m))
}

fn weekday_num(s: &str) -> Option<u8> {
    match s.to_ascii_lowercase().as_str() {
        "sun" | "sunday" => Some(0),
        "mon" | "monday" => Some(1),
        "tue" | "tuesday" => Some(2),
        "wed" | "wednesday" => Some(3),
        "thu" | "thursday" => Some(4),
        "fri" | "friday" => Some(5),
        "sat" | "saturday" => Some(6),
        _ => None,
    }
}

pub fn parse_calendar(s: &str) -> Result<Schedule> {
    let s = s.trim();
    match s.to_ascii_lowercase().as_str() {
        "hourly" => return Ok(Schedule::Hourly { minute: 0 }),
        "daily" => return Ok(Schedule::Daily { hour: 0, minute: 0 }),
        "weekly" => return Ok(Schedule::Weekly { weekday: 1, hour: 0, minute: 0 }),
        _ => {}
    }
    // "HH:MM"
    if let Some((h, m)) = parse_hm(s) {
        return Ok(Schedule::Daily { hour: h, minute: m });
    }
    // "*-*-* HH:MM:SS"
    if let Some(rest) = s.strip_prefix("*-*-*") {
        if let Some((h, m)) = parse_hms(rest.trim()) {
            return Ok(Schedule::Daily { hour: h, minute: m });
        }
    }
    // "<Weekday> *-*-* HH:MM:SS"
    if let Some((day, rest)) = s.split_once(' ') {
        if let Some(wd) = weekday_num(day) {
            if let Some(rest) = rest.trim().strip_prefix("*-*-*") {
                if let Some((h, m)) = parse_hms(rest.trim()) {
                    return Ok(Schedule::Weekly { weekday: wd, hour: h, minute: m });
                }
            }
        }
    }
    Err(KgError::Other(format!("unsupported calendar spec {s:?}; {SUPPORTED}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_keywords() {
        assert_eq!(parse_calendar("hourly").unwrap(), Schedule::Hourly { minute: 0 });
        assert_eq!(parse_calendar("daily").unwrap(), Schedule::Daily { hour: 0, minute: 0 });
        assert_eq!(parse_calendar("weekly").unwrap(), Schedule::Weekly { weekday: 1, hour: 0, minute: 0 });
    }

    #[test]
    fn parses_hh_mm() {
        assert_eq!(parse_calendar("03:00").unwrap(), Schedule::Daily { hour: 3, minute: 0 });
        assert_eq!(parse_calendar("23:59").unwrap(), Schedule::Daily { hour: 23, minute: 59 });
    }

    #[test]
    fn parses_systemd_daily() {
        assert_eq!(parse_calendar("*-*-* 03:00:00").unwrap(), Schedule::Daily { hour: 3, minute: 0 });
    }

    #[test]
    fn parses_systemd_weekday() {
        assert_eq!(
            parse_calendar("Mon *-*-* 09:30:00").unwrap(),
            Schedule::Weekly { weekday: 1, hour: 9, minute: 30 }
        );
        assert_eq!(
            parse_calendar("sun *-*-* 00:00:00").unwrap(),
            Schedule::Weekly { weekday: 0, hour: 0, minute: 0 }
        );
    }

    #[test]
    fn rejects_unsupported_with_helpful_error() {
        for bad in ["*/5 * * * *", "monthly", "25:00", "*-*-* 99:00:00", "Mon..Fri 09:00"] {
            let err = parse_calendar(bad).unwrap_err().to_string();
            assert!(err.contains("supported calendar forms"), "error for {bad:?} should list supported forms, got: {err}");
        }
    }
}
```

- [ ] **Step 2: Wire the module and run tests**

Add `pub mod calendar;` to `crates/kgx-cron/src/lib.rs`.
Run: `cargo test -p kgx-cron calendar`
Expected: PASS (module is self-contained).

- [ ] **Step 3: Write failing tests for the launchd renderer** — in `crates/kgx-cron/src/unit.rs`, replace the existing `launchd_plist_has_calendar_interval` test and add coverage:

```rust
#[test]
fn launchd_plist_daily_hh_mm() {
    let j = Job { name: "dream-nightly".into(), command: "kg dream".into(), calendar: "03:00".into() };
    let plist = render_launchd(&j).unwrap();
    assert!(plist.contains("StartCalendarInterval"));
    assert!(plist.contains("<key>Hour</key><integer>3</integer>"));
    assert!(plist.contains("<key>Minute</key><integer>0</integer>"));
}

#[test]
fn launchd_plist_hourly_has_no_hour_key() {
    let j = Job { name: "gc".into(), command: "kg index".into(), calendar: "hourly".into() };
    let plist = render_launchd(&j).unwrap();
    assert!(plist.contains("<key>Minute</key><integer>0</integer>"));
    assert!(!plist.contains("<key>Hour</key>"), "hourly must omit Hour so launchd fires every hour");
}

#[test]
fn launchd_plist_weekly_has_weekday() {
    let j = Job { name: "wk".into(), command: "kg dream".into(), calendar: "Mon *-*-* 09:30:00".into() };
    let plist = render_launchd(&j).unwrap();
    assert!(plist.contains("<key>Weekday</key><integer>1</integer>"));
    assert!(plist.contains("<key>Hour</key><integer>9</integer>"));
    assert!(plist.contains("<key>Minute</key><integer>30</integer>"));
}

#[test]
fn launchd_rejects_unsupported_instead_of_malformed_plist() {
    let j = Job { name: "bad".into(), command: "kg dream".into(), calendar: "*/5 * * * *".into() };
    assert!(render_launchd(&j).is_err());
}

#[test]
fn systemd_syntax_no_longer_renders_malformed_launchd_hour() {
    let j = Job { name: "sysd".into(), command: "kg dream".into(), calendar: "*-*-* 03:00:00".into() };
    let plist = render_launchd(&j).unwrap();
    assert!(plist.contains("<key>Hour</key><integer>3</integer>"));
    assert!(!plist.contains("*-*-*"), "raw systemd tokens must never leak into a plist");
}
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test -p kgx-cron launchd`
Expected: FAIL — `render_launchd` returns `String`, not `Result`, and mis-renders non-HH:MM input.

- [ ] **Step 5: Rewrite `render_launchd`** in `crates/kgx-cron/src/unit.rs`:

```rust
use crate::calendar::{parse_calendar, Schedule};
use kgx_core::Result;

pub fn render_launchd(j: &Job) -> Result<String> {
    let sched = parse_calendar(&j.calendar)?;
    let interval = match sched {
        Schedule::Hourly { minute } => format!("<dict><key>Minute</key><integer>{minute}</integer></dict>"),
        Schedule::Daily { hour, minute } => format!(
            "<dict><key>Hour</key><integer>{hour}</integer><key>Minute</key><integer>{minute}</integer></dict>"
        ),
        Schedule::Weekly { weekday, hour, minute } => format!(
            "<dict><key>Weekday</key><integer>{weekday}</integer><key>Hour</key><integer>{hour}</integer><key>Minute</key><integer>{minute}</integer></dict>"
        ),
    };
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\"><dict>\n\
  <key>Label</key><string>sh.kgx.{name}</string>\n\
  <key>ProgramArguments</key><array><string>/bin/sh</string><string>-lc</string><string>{cmd}</string></array>\n\
  <key>StartCalendarInterval</key>{interval}\n\
</dict></plist>\n",
        name = j.name,
        cmd = j.command,
    ))
}
```

- [ ] **Step 6: Fix callers.** In `crates/kgx-cron/src/manage.rs::add`, the macOS arm becomes `std::fs::write(&p, render_launchd(job)?)...`. Also add cross-platform validation at the top of `add()` so Linux gets the same early error:

```rust
pub fn add(job: &Job) -> Result<Vec<PathBuf>> {
    // Validate the calendar on every platform before writing any file.
    crate::calendar::parse_calendar(&job.calendar)?;
    ...
```

Note: `render_systemd` keeps passing the raw string through — systemd natively accepts every form `parse_calendar` admits (`hourly`, `daily`, `weekly`, `HH:MM`, `*-*-* HH:MM:SS`, `Mon *-*-* HH:MM:SS`).

- [ ] **Step 7: Run tests**

Run: `cargo test -p kgx-cron && cargo test --workspace`
Expected: PASS (the pre-existing `launchd_plist_has_calendar_interval` test was replaced in Step 3).

- [ ] **Step 8: Commit**

```bash
git add crates/kgx-cron/src/calendar.rs crates/kgx-cron/src/unit.rs crates/kgx-cron/src/lib.rs crates/kgx-cron/src/manage.rs
git commit -m "feat(ws5): parse systemd-style calendars for launchd; reject unsupported specs

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: WS5 — `kg cron remove`

**Files:**
- Modify: `crates/kgx-cron/src/manage.rs`
- Modify: `crates/kgx-cli/src/commands/cron.rs`
- Test: `crates/kgx-cron/src/manage.rs` (inline)

**Interfaces:**
- Produces: `pub fn remove(name: &str) -> Result<Vec<PathBuf>>` — returns the deleted file paths; errors if no unit files exist for `name`.
- Consumes: existing `disable()` (best-effort), `systemd_dir()`, `launchd_dir()`.

- [ ] **Step 1: Write the failing test** — append to `crates/kgx-cron/src/manage.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_deletes_unit_files_and_errors_when_absent() {
        let tmp = tempfile::tempdir().unwrap();
        // Route HOME/XDG so unit dirs land inside the tempdir on any platform.
        std::env::set_var("HOME", tmp.path());
        std::env::set_var("XDG_CONFIG_HOME", tmp.path().join(".config"));

        let job = Job {
            name: "rmtest".into(),
            command: "kg dream".into(),
            calendar: "03:00".into(),
        };
        let written = add(&job).unwrap();
        assert!(!written.is_empty());
        for f in &written {
            assert!(f.exists());
        }

        let deleted = remove("rmtest").unwrap();
        assert_eq!(deleted.len(), written.len());
        for f in &deleted {
            assert!(!f.exists());
        }

        let err = remove("rmtest").unwrap_err().to_string();
        assert!(err.contains("rmtest"), "error should name the missing unit: {err}");
    }
}
```

Add `tempfile.workspace = true` under `[dev-dependencies]` in `crates/kgx-cron/Cargo.toml` if not present.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kgx-cron remove_deletes`
Expected: FAIL — `remove` not found.

- [ ] **Step 3: Implement `remove`** in `crates/kgx-cron/src/manage.rs`:

```rust
/// Delete the unit files for `name` (after a best-effort disable).
/// `disable` keeps the files; `remove` deletes them.
pub fn remove(name: &str) -> Result<Vec<PathBuf>> {
    let candidates: Vec<PathBuf> = match Platform::detect() {
        Platform::Linux => vec![
            systemd_dir().join(format!("kgx-{name}.service")),
            systemd_dir().join(format!("kgx-{name}.timer")),
        ],
        Platform::Macos => vec![launchd_dir().join(format!("sh.kgx.{name}.plist"))],
        Platform::Other => return Err(KgError::Other("unsupported platform for cron".into())),
    };
    let existing: Vec<PathBuf> = candidates.into_iter().filter(|p| p.exists()).collect();
    if existing.is_empty() {
        return Err(KgError::Other(format!(
            "no cron unit named '{name}' — see `kg cron list`"
        )));
    }
    let _ = disable(name); // best-effort unload; files may never have been enabled
    let mut deleted = Vec::new();
    for p in existing {
        std::fs::remove_file(&p).map_err(|e| KgError::Io {
            path: p.display().to_string(),
            source: e,
        })?;
        deleted.push(p);
    }
    Ok(deleted)
}
```

- [ ] **Step 4: Wire the CLI.** In `crates/kgx-cli/src/commands/cron.rs`, add an arm before the `other =>` catch-all:

```rust
"remove" => {
    let files = manage::remove(&name.ok_or_else(|| anyhow::anyhow!("cron remove requires a name"))?)?;
    emit(
        "cron",
        serde_json::json!({"removed": files.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()}),
        json,
        start,
        |d| println!("removed {} unit file(s)", d["removed"].as_array().map(|a| a.len()).unwrap_or(0)),
    );
}
```

(`action` is a plain `String` in clap, so no `cli.rs` change is needed.)

- [ ] **Step 5: Run tests**

Run: `cargo test -p kgx-cron && cargo build -p kgx-cli`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/kgx-cron/src/manage.rs crates/kgx-cron/Cargo.toml crates/kgx-cli/src/commands/cron.rs
git commit -m "feat(ws5): kg cron remove deletes unit files (disable keeps them)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: WS6 — Make `review --reject` real; resolved diffs leave the staged file

**Files:**
- Modify: `crates/kgx-cli/src/commands/review.rs`
- Test: `crates/kgx-cli/src/commands/review.rs` (inline, pure resolution logic)

**Interfaces:**
- Produces: `pub(crate) enum Action { Apply, Reject, Keep }`, `pub(crate) fn resolve_action(diff: &ProposedDiff, approve_all: bool, approve_ids: &BTreeSet<String>, reject_all: bool, reject_ids: &BTreeSet<String>) -> Action`. After `run()`, `.kg/staged_diffs.json` contains only unresolved diffs; resolutions append to `.kg/review-log.jsonl` as `{"ts","id","pass","action"}` lines. Task 6 (interactive) reuses `resolve_action` semantics by materializing per-diff choices into `approve_ids`/`reject_ids`.
- Consumes: `kgx_core::diff::{ProposedDiff, Severity}`.

- [ ] **Step 1: Write the failing tests** — append to `crates/kgx-cli/src/commands/review.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use kgx_core::diff::{DiffKind, ProposedDiff, Severity};

    fn diff(id: &str, sev: Severity) -> ProposedDiff {
        ProposedDiff {
            id: id.into(),
            pass: "dedup".into(),
            kind: DiffKind::Merge,
            severity: sev,
            rationale: "t".into(),
            files: vec![],
        }
    }

    #[test]
    fn reject_ids_win_over_approve_all() {
        let d = diff("A", Severity::Soft);
        let rej: BTreeSet<String> = ["A".to_string()].into();
        assert!(matches!(
            resolve_action(&d, true, &BTreeSet::new(), false, &rej),
            Action::Reject
        ));
    }

    #[test]
    fn approve_all_skips_hard_but_explicit_id_applies() {
        let d = diff("H", Severity::Hard);
        assert!(matches!(
            resolve_action(&d, true, &BTreeSet::new(), false, &BTreeSet::new()),
            Action::Keep
        ));
        let ids: BTreeSet<String> = ["H".to_string()].into();
        assert!(matches!(
            resolve_action(&d, false, &ids, false, &BTreeSet::new()),
            Action::Apply
        ));
    }

    #[test]
    fn reject_all_rejects_everything_including_hard() {
        let d = diff("H", Severity::Hard);
        assert!(matches!(
            resolve_action(&d, false, &BTreeSet::new(), true, &BTreeSet::new()),
            Action::Reject
        ));
    }

    #[test]
    fn untouched_diffs_are_kept() {
        let d = diff("X", Severity::Soft);
        assert!(matches!(
            resolve_action(&d, false, &BTreeSet::new(), false, &BTreeSet::new()),
            Action::Keep
        ));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p kgx-cli resolve_action`
Expected: FAIL — `Action` / `resolve_action` not found.

- [ ] **Step 3: Implement.** In `crates/kgx-cli/src/commands/review.rs`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    Apply,
    Reject,
    Keep,
}

pub(crate) fn resolve_action(
    diff: &ProposedDiff,
    approve_all: bool,
    approve_ids: &BTreeSet<String>,
    reject_all: bool,
    reject_ids: &BTreeSet<String>,
) -> Action {
    if reject_all || reject_ids.contains(&diff.id) {
        return Action::Reject;
    }
    if approve_ids.contains(&diff.id) {
        return Action::Apply;
    }
    if approve_all && !matches!(diff.severity, Severity::Hard) {
        return Action::Apply;
    }
    Action::Keep
}
```

Then rewrite the body of `run()` (rename `_reject` → `reject`):

```rust
let approve_all = approve.as_deref() == Some("all");
let approve_ids: BTreeSet<String> = ids_from(approve.as_deref());
let reject_all = reject.as_deref() == Some("all");
let reject_ids: BTreeSet<String> = ids_from(reject.as_deref());

let mut applied = 0u32;
let mut rejected = 0u32;
let mut blocked_hard = 0u32;
let mut audit_flags = Vec::new();
let mut remaining: Vec<ProposedDiff> = Vec::new();
let mut log_lines: Vec<String> = Vec::new();

for diff in staged {
    match resolve_action(&diff, approve_all, &approve_ids, reject_all, &reject_ids) {
        Action::Reject => {
            rejected += 1;
            log_lines.push(review_log_line(&diff, "reject"));
        }
        Action::Apply => {
            if ponytail_audit {
                for flag in kgx_ponytail::audit_diff(&diff) {
                    audit_flags.push(format!("{}: {}", flag.code, flag.msg));
                }
            }
            for file in &diff.files {
                if let Some(after) = &file.after {
                    let path = root.join(&file.rel_path);
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(path, after)?;
                }
            }
            applied += 1;
            log_lines.push(review_log_line(&diff, "apply"));
        }
        Action::Keep => {
            if matches!(diff.severity, Severity::Hard) && approve_all {
                blocked_hard += 1;
            }
            remaining.push(diff);
        }
    }
}

// Resolved diffs leave the staged file; unresolved stay.
std::fs::create_dir_all(root.join(".kg"))?;
std::fs::write(&staged_path, serde_json::to_string_pretty(&remaining)?)?;
if !log_lines.is_empty() {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join(".kg/review-log.jsonl"))?;
    for l in &log_lines {
        writeln!(f, "{l}")?;
    }
}
```

with helpers:

```rust
fn ids_from(v: Option<&str>) -> BTreeSet<String> {
    v.filter(|s| *s != "all")
        .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
        .unwrap_or_default()
}

fn review_log_line(diff: &ProposedDiff, action: &str) -> String {
    serde_json::json!({
        "ts": kgx_core::util::now_iso(),
        "id": diff.id,
        "pass": diff.pass,
        "action": action,
    })
    .to_string()
}
```

Update the final `emit` payload to include `"rejected": rejected, "remaining": remaining_count` (capture `let remaining_count = remaining.len();` before the write) and the human line to `println!("applied {applied}; rejected {rejected}; {blocked_hard} hard blocked; {remaining} left staged")` style. `ProposedDiff` must be `Serialize` — it already round-trips through `staged_diffs.json`, so it is.

- [ ] **Step 4: Run tests**

Run: `cargo test -p kgx-cli && cargo test --workspace`
Expected: PASS. If a smoke test asserts the old `review` output line, update it for the new fields.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-cli/src/commands/review.rs
git commit -m "feat(ws6): review --reject discards staged diffs; resolved diffs leave the staged file

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: WS6 — Minimal interactive review loop

**Files:**
- Modify: `crates/kgx-cli/src/commands/review.rs`

**Interfaces:**
- Produces: `pub(crate) fn interactive_choices(staged: &[ProposedDiff], input: &mut dyn std::io::BufRead, out: &mut dyn std::io::Write) -> std::io::Result<(BTreeSet<String>, BTreeSet<String>)>` — returns `(approve_ids, reject_ids)`; `q` stops early leaving the rest untouched; `s`/empty skips.
- Consumes: Task 5's `resolve_action` path (the returned sets feed the same resolution loop).

- [ ] **Step 1: Write the failing test** — add to the `tests` module in `review.rs`:

```rust
#[test]
fn interactive_collects_choices_and_quits_early() {
    let staged = vec![
        diff("A", Severity::Soft),
        diff("B", Severity::Soft),
        diff("C", Severity::Hard),
        diff("D", Severity::Soft),
    ];
    // approve A, reject B, skip C, quit before D
    let mut input = std::io::Cursor::new(b"a\nr\ns\nq\n".to_vec());
    let mut out = Vec::new();
    let (approve, reject) = interactive_choices(&staged, &mut input, &mut out).unwrap();
    assert_eq!(approve, ["A".to_string()].into());
    assert_eq!(reject, ["B".to_string()].into());
    let shown = String::from_utf8(out).unwrap();
    assert!(shown.contains("[1/4]"));
    assert!(shown.contains("hard"), "severity must be shown");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kgx-cli interactive_collects`
Expected: FAIL — `interactive_choices` not found.

- [ ] **Step 3: Implement**:

```rust
pub(crate) fn interactive_choices(
    staged: &[ProposedDiff],
    input: &mut dyn std::io::BufRead,
    out: &mut dyn std::io::Write,
) -> std::io::Result<(BTreeSet<String>, BTreeSet<String>)> {
    let mut approve = BTreeSet::new();
    let mut reject = BTreeSet::new();
    let total = staged.len();
    for (i, d) in staged.iter().enumerate() {
        writeln!(
            out,
            "[{}/{}] {} · {:?} · severity={:?}\n  {}\n  files: {}",
            i + 1,
            total,
            d.pass,
            d.kind,
            d.severity,
            d.rationale,
            d.files.iter().map(|f| f.rel_path.as_str()).collect::<Vec<_>>().join(", ")
        )?;
        write!(out, "  [a]pprove / [r]eject / [s]kip / [q]uit > ")?;
        out.flush()?;
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            break; // EOF = quit
        }
        match line.trim().to_ascii_lowercase().as_str() {
            "a" => {
                approve.insert(d.id.clone());
            }
            "r" => {
                reject.insert(d.id.clone());
            }
            "q" => break,
            _ => {} // skip
        }
    }
    Ok((approve, reject))
}
```

Note: `severity={:?}` debug-prints e.g. `Hard`; lowercase it in the format if the assert needs it — simplest is `format!("{:?}", d.severity).to_lowercase()` interpolated. `FileChange.rel_path` is a `String` (see `contradiction.rs` usage); if the compiler disagrees, use `.rel_path.clone()`.

- [ ] **Step 4: Wire into `run()`.** Replace the current TTY-check block:

```rust
let (mut approve, mut reject) = (approve, reject);
if interactive {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("--interactive requires a terminal (stdin is not a TTY)");
    }
    let staged_now: Vec<ProposedDiff> = if staged_path.exists() {
        serde_json::from_str(&std::fs::read_to_string(&staged_path)?)?
    } else {
        vec![]
    };
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout();
    let (a, r) = interactive_choices(&staged_now, &mut stdin, &mut stdout)?;
    approve = Some(a.into_iter().collect::<Vec<_>>().join(","));
    reject = Some(r.into_iter().collect::<Vec<_>>().join(","));
}
```

(Reorder `run()` so `staged_path` is computed before this block; the main resolution loop then proceeds exactly as in Task 5.)

- [ ] **Step 5: Run tests**

Run: `cargo test -p kgx-cli && cargo test --workspace`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/kgx-cli/src/commands/review.rs
git commit -m "feat(ws6): minimal interactive review loop (approve/reject/skip/quit)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 7: WS8 — Harness parity for Cursor, Codex, Opencode

**Files:**
- Modify: `skills/cursor/.cursor/mcp.json`
- Modify: `skills/codex/config.toml`
- Modify: `skills/opencode/opencode.json`
- Create: `skills/cursor/.cursor/rules/kgx-codebase.mdc`
- Create: `skills/cursor/.cursor/rules/kgx-finish.mdc`

**Interfaces:**
- Produces: all four harness configs register both `kgx` and `codebase-memory-mcp`; Cursor gains codebase + finish-gate rules. Task 9's parity test enforces this shape.

- [ ] **Step 1: Update Cursor MCP config** — `skills/cursor/.cursor/mcp.json`:

```json
{ "mcpServers": {
  "kgx": { "command": "kg", "args": ["mcp-server", "--transport", "stdio"] },
  "codebase-memory-mcp": { "command": "codebase-memory-mcp" }
} }
```

- [ ] **Step 2: Update Codex MCP config** — append to `skills/codex/config.toml`:

```toml

[mcp_servers.codebase-memory-mcp]
command = "codebase-memory-mcp"
args = []
```

- [ ] **Step 3: Update Opencode MCP config** — `skills/opencode/opencode.json`:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "kgx": {
      "type": "local",
      "command": ["kg", "mcp-server", "--transport", "stdio"]
    },
    "codebase-memory-mcp": {
      "type": "local",
      "command": ["codebase-memory-mcp"]
    }
  }
}
```

- [ ] **Step 4: Create the Cursor codebase rule** — `skills/cursor/.cursor/rules/kgx-codebase.mdc`. Mirror the tool list from `skills/claude/.claude/skills/kgx-codebase/SKILL.md` (read it first; keep tool names identical):

```markdown
---
description: Codebase knowledge graph via codebase-memory-mcp
alwaysApply: false
---

# kgx-codebase

Use the codebase-memory-mcp MCP server for structural code questions before
falling back to text search.

- `index_repository` — index the repo first if not yet indexed
- `search_graph` — find functions/classes/routes by name/label/pattern
- `trace_path` — call chains and data flow between symbols
- `get_code_snippet` — exact source for a qualified symbol name
- `query_graph` — Cypher-style structural queries
- `get_architecture` — project structure overview
- `search_code` — graph-augmented text search

Vault knowledge ("why we built X") lives in the kgx MCP server; code structure
("where X is implemented") lives here. Use both when answering design questions.
```

- [ ] **Step 5: Create the Cursor finish rule** — `skills/cursor/.cursor/rules/kgx-finish.mdc`:

```markdown
---
description: Run the shared KGX finish gate before declaring work done
alwaysApply: true
---

# kgx finish gate

Before telling the user a task is complete, run:

    sh "$(git rev-parse --show-toplevel)/.kgx/hooks/verify-finished.sh" --json

If it fails, inspect `.kgx/hooks/last-finish-check.log`, fix the failure, and
run the check again before responding. This mirrors the verify-finished Stop
hook that Claude Code and Codex enforce automatically.
```

- [ ] **Step 6: Validate JSON/TOML syntax**

Run: `python3 -c "import json;[json.load(open(p)) for p in ['skills/cursor/.cursor/mcp.json','skills/opencode/opencode.json','skills/claude/.mcp.json']]" && python3 -c "import tomllib;tomllib.load(open('skills/codex/config.toml','rb'))"`
Expected: no output (all parse).

- [ ] **Step 7: Run the existing skills test**

Run: `cargo test -p smoke t_skills_valid`
Expected: PASS (additions don't break the current assertions; Task 9 tightens them).

- [ ] **Step 8: Commit**

```bash
git add skills/cursor skills/codex/config.toml skills/opencode/opencode.json
git commit -m "feat(ws8): register codebase-memory-mcp in all harness configs; add cursor codebase + finish rules

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 8: WS8 — ZCode harness pack + installer (and fix the Opencode installer branch)

**Files:**
- Create: `skills/zcode/.mcp.json`
- Create: `skills/zcode/.zcode/skills/kgx/SKILL.md` (copy of `skills/claude/.claude/skills/kgx/SKILL.md`)
- Create: `skills/zcode/.zcode/skills/kgx-codebase/SKILL.md` (copy of claude's)
- Create: `skills/zcode/.zcode/skills/kgx-codebase-index/SKILL.md` (copy of claude's)
- Modify: `dev-install.sh`
- Modify: `README.md` (harness matrix)

**Interfaces:**
- Produces: `./dev-install.sh --agent zcode` installs MCP config + mirrors skills to `~/.zcode/skills/`; `skills/zcode/` is the in-repo template Task 9's parity test reads. ZCode uses the same MCP-stdio contract as Cursor/Claude (`.mcp.json` with `mcpServers`).

- [ ] **Step 1: Create the ZCode MCP template** — `skills/zcode/.mcp.json`:

```json
{ "mcpServers": {
  "kgx": { "command": "kg", "args": ["mcp-server", "--transport", "stdio"] },
  "codebase-memory-mcp": { "command": "codebase-memory-mcp" }
} }
```

- [ ] **Step 2: Mirror the skill pack**

```bash
mkdir -p skills/zcode/.zcode/skills/{kgx,kgx-codebase,kgx-codebase-index}
cp skills/claude/.claude/skills/kgx/SKILL.md skills/zcode/.zcode/skills/kgx/SKILL.md
cp skills/claude/.claude/skills/kgx-codebase/SKILL.md skills/zcode/.zcode/skills/kgx-codebase/SKILL.md
cp skills/claude/.claude/skills/kgx-codebase-index/SKILL.md skills/zcode/.zcode/skills/kgx-codebase-index/SKILL.md
```

If any copied SKILL.md contains Claude-specific wording (e.g. references to `/kgx:` slash commands or `.claude/` paths), replace those references with harness-neutral wording ("your agent's skill invocation") — check with `grep -n "claude\|/kgx:" skills/zcode/.zcode/skills/*/SKILL.md`.

- [ ] **Step 3: Add the zcode installer branch** — in `dev-install.sh`:

Line 3 usage comment → `# Usage: ./dev-install.sh [--agent claude|opencode|codex|cursor|zcode] [--vault ~/path/to/vault]`
Line 22 validation → `claude|opencode|codex|cursor|zcode) ;;`
Line 23 error → `*) echo "Invalid --agent: $AGENT (choose: claude, opencode, codex, cursor, zcode)"; exit 1 ;;`

New case branch (alongside `cursor)`), same structure as the others:

```sh
  zcode)
    cp "$REPO_DIR/skills/zcode/.mcp.json" "$VAULT_DIR/.mcp.json"
    ok "copied .mcp.json -> $VAULT_DIR/.mcp.json (kgx + codebase-memory-mcp, stdio)"
    for s in kgx kgx-codebase kgx-codebase-index; do
      mkdir -p "${HOME}/.zcode/skills/$s"
      cp "$REPO_DIR/skills/zcode/.zcode/skills/$s/SKILL.md" "${HOME}/.zcode/skills/$s/SKILL.md"
    done
    ok "mirrored kgx skills -> ~/.zcode/skills/"
    ;;
```

- [ ] **Step 4: Fix the Opencode installer branch.** Replace the heredoc at `dev-install.sh` lines ~127–138 (the one writing `~/.config/opencode/opencode.json` with a hardcoded `/usr/local/bin/kg` and no `--transport stdio`) with an expanding heredoc that matches the in-repo template:

```sh
    cat > "${HOME}/.config/opencode/opencode.json" << OPENCODE_EOF
{
  "\$schema": "https://opencode.ai/config.json",
  "mcp": {
    "kgx": {
      "type": "local",
      "command": ["$BIN_DIR/kg", "mcp-server", "--transport", "stdio"]
    },
    "codebase-memory-mcp": {
      "type": "local",
      "command": ["$BIN_DIR/codebase-memory-mcp"]
    }
  }
}
OPENCODE_EOF
```

(Unquoted delimiter so `$BIN_DIR` expands; `\$schema` stays literal.)

- [ ] **Step 5: Update the README harness matrix.** Find the supported-harness section (`grep -n "Cursor\|Opencode\|harness" README.md`) and add a ZCode row/entry stating: MCP via `.mcp.json` (stdio), skills mirrored to `~/.zcode/skills/`, installed via `./dev-install.sh --agent zcode`.

- [ ] **Step 6: Verify installer syntax and dry behavior**

Run: `bash -n dev-install.sh && python3 -c "import json;json.load(open('skills/zcode/.mcp.json'))"`
Expected: no output.

- [ ] **Step 7: Commit**

```bash
git add skills/zcode dev-install.sh README.md
git commit -m "feat(ws8): ZCode native harness pack + installer; fix opencode installer path/transport

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 9: WS8 — Structural parity test across all 5 harnesses

**Files:**
- Modify: `tests/smoke/tests/t_skills_valid.rs`

**Interfaces:**
- Consumes: files created in Tasks 7–8.
- Produces: CI-enforced parity — a drifting harness config fails `cargo test -p smoke`.

- [ ] **Step 1: Extend the test** — in `native_skill_packages_reference_same_mcp_tools`:

Add to the `files` array (skill/rule docs that must name all 9 tools):

```rust
root.join("skills/zcode/.zcode/skills/kgx/SKILL.md"),
```

Add to the MCP-config loop array:

```rust
root.join("skills/zcode/.mcp.json"),
```

Then add a new assertion loop at the end of the test:

```rust
// Every harness must register the codebase MCP server, not just Claude.
for config in [
    root.join("skills/claude/.mcp.json"),
    root.join("skills/codex/config.toml"),
    root.join("skills/cursor/.cursor/mcp.json"),
    root.join("skills/opencode/opencode.json"),
    root.join("skills/zcode/.mcp.json"),
] {
    let text = std::fs::read_to_string(&config)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", config.display()));
    assert!(
        text.contains("codebase-memory-mcp"),
        "{} missing codebase-memory-mcp registration",
        config.display()
    );
}
// Cursor's finish gate ships as an alwaysApply rule.
let cursor_finish = root.join("skills/cursor/.cursor/rules/kgx-finish.mdc");
let text = std::fs::read_to_string(&cursor_finish)
    .unwrap_or_else(|e| panic!("failed to read {}: {e}", cursor_finish.display()));
assert!(text.contains("verify-finished"));
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p smoke t_skills_valid`
Expected: PASS (Tasks 7–8 created every file). Temporarily rename `skills/zcode/.mcp.json` and re-run to confirm it FAILS, then restore — that proves the gate bites.

- [ ] **Step 3: Commit**

```bash
git add tests/smoke/tests/t_skills_valid.rs
git commit -m "test(ws8): enforce 5-harness MCP/skill parity structurally

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 10: WS2 — `EntityType` + typed `RelType` variants in kgx-core

**Files:**
- Modify: `crates/kgx-core/src/types.rs`
- Modify: `crates/kgx-core/src/lib.rs` (re-export `EntityType` alongside the existing type re-exports — check with `grep -n "pub use" crates/kgx-core/src/lib.rs` and match the pattern)

**Interfaces:**
- Produces:
  - `pub enum EntityType { Person, Object, Location, Event }` with `pub fn parse(s: &str) -> Option<EntityType>` and `pub fn as_str(self) -> &'static str`.
  - New `RelType` variants: `ParticipatesIn, LocatedAt, Owns, Decided, Caused` and `pub fn parse(s: &str) -> Option<RelType>` (snake_case names).
  - `Frontmatter.entity_type` stays `Option<String>` (OKF tolerance for non-POLE values like `system` in existing vaults); `EntityType` is the classification vocabulary used by extract/brain/viz.

- [ ] **Step 1: Write the failing tests** — add to the `tests` module in `types.rs`:

```rust
#[test]
fn entity_type_parses_pole_only() {
    assert_eq!(EntityType::parse("person"), Some(EntityType::Person));
    assert_eq!(EntityType::parse("OBJECT"), Some(EntityType::Object));
    assert_eq!(EntityType::parse("location"), Some(EntityType::Location));
    assert_eq!(EntityType::parse("event"), Some(EntityType::Event));
    assert_eq!(EntityType::parse("system"), None);
    assert_eq!(EntityType::Person.as_str(), "person");
}

#[test]
fn rel_type_parses_snake_case_typed_relations() {
    assert_eq!(RelType::parse("participates_in"), Some(RelType::ParticipatesIn));
    assert_eq!(RelType::parse("located_at"), Some(RelType::LocatedAt));
    assert_eq!(RelType::parse("owns"), Some(RelType::Owns));
    assert_eq!(RelType::parse("decided"), Some(RelType::Decided));
    assert_eq!(RelType::parse("caused"), Some(RelType::Caused));
    assert_eq!(RelType::parse("links_to"), Some(RelType::LinksTo));
    assert_eq!(RelType::parse("nonsense"), None);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p kgx-core entity_type -- --nocapture; cargo test -p kgx-core rel_type`
Expected: FAIL — types/methods missing.

- [ ] **Step 3: Implement** in `types.rs`:

```rust
/// POLE classification for entity notes (Person / Object / Location / Event).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityType {
    Person,
    Object,
    Location,
    Event,
}

impl EntityType {
    pub fn parse(s: &str) -> Option<EntityType> {
        match s.to_ascii_lowercase().as_str() {
            "person" => Some(Self::Person),
            "object" => Some(Self::Object),
            "location" => Some(Self::Location),
            "event" => Some(Self::Event),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Object => "object",
            Self::Location => "location",
            Self::Event => "event",
        }
    }
}
```

Extend `RelType`:

```rust
pub enum RelType {
    LinksTo,
    Supersedes,
    DerivedFrom,
    Cites,
    MentionsEntity,
    Contradicts,
    ParticipatesIn,
    LocatedAt,
    Owns,
    Decided,
    Caused,
}

impl RelType {
    pub fn parse(s: &str) -> Option<RelType> {
        serde_json::from_value(serde_json::Value::String(s.to_string())).ok()
    }
}
```

(`RelType` already derives `Deserialize` with `rename_all = "snake_case"`, so `parse` piggybacks on serde and can never drift from the serialized names. `serde_json` is already a kgx-core dependency — verify with `grep serde_json crates/kgx-core/Cargo.toml`; if absent, add `serde_json.workspace = true`.)

Add `EntityType` to the lib.rs re-export line where `NoteType`, `RelType`, etc. are exported.

- [ ] **Step 4: Run tests**

Run: `cargo test -p kgx-core && cargo test --workspace`
Expected: PASS — new variants are additive; exhaustive matches on `RelType` elsewhere (if any) will surface as compile errors; fix them by adding arms that map the new variants like `LinksTo` is handled.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-core/src/types.rs crates/kgx-core/src/lib.rs
git commit -m "feat(ws2): EntityType (POLE) enum + typed RelType variants

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 11: WS2 — POLE-aware extract prompt + pipeline

**Files:**
- Modify: `crates/kgx-extract/src/prompt.rs`
- Modify: `crates/kgx-extract/src/pipeline.rs`
- Modify: `crates/kgx-extract/Cargo.toml` (add `[dev-dependencies] tokio = { version = "1", features = ["rt-multi-thread", "macros"] }`)
- Test: `crates/kgx-extract/tests/pole.rs`

**Interfaces:**
- Consumes: `kgx_core::EntityType` (Task 10).
- Produces: extract emits, per LLM response: fact notes (as today, plus `extra["relations"]`) **and** entity notes (`NoteType::Entity`, `entity_type` set, `rel_path` `notes/entities/<slug>.md`). New JSON contract accepted from the LLM:
  `{"facts":[{"title","body","confidence","entities":[{"name","entity_type","rel"} | "bare-string"]}]}`
  where `entity_type ∈ person|object|location|event` (optional) and `rel ∈ mentions|participates_in|located_at|owns|decided|caused` (optional, default `mentions`). Bare-string entities (old contract) remain valid — POLE is additive, mock-LLM output still works.
- Fact note `extra["relations"]`: YAML sequence of mappings `{ target: <entity title>, rel: <rel> }`, only for `rel != mentions`. Task 12's `derive_edges` consumes this key.

- [ ] **Step 1: Update the system prompt** — `crates/kgx-extract/src/prompt.rs`:

```rust
pub const EXTRACT_SYSTEM: &str = "You extract atomic, one-claim-per-note facts with provenance, and classify referenced entities with the POLE taxonomy. Reply JSON {facts:[{title,body,confidence,entities:[{name,entity_type,rel}]}]}. entity_type is one of person|object|location|event. rel describes how the fact relates to the entity: one of mentions|participates_in|located_at|owns|decided|caused (default mentions). If unsure of entity_type, omit it.";
```

- [ ] **Step 2: Write the failing contract test** — create `crates/kgx-extract/tests/pole.rs`:

```rust
use kgx_core::llm::{LlmProvider, LlmRequest, LlmResponse};
use kgx_core::{EntityType, NoteType};
use kgx_extract::pipeline::{extract, Intensity};

struct StubProvider(String);

#[async_trait::async_trait]
impl LlmProvider for StubProvider {
    async fn complete(&self, _req: LlmRequest) -> kgx_core::Result<LlmResponse> {
        Ok(LlmResponse {
            text: self.0.clone(),
            input_tokens: 1,
            output_tokens: 1,
            model: "stub".into(),
        })
    }
    fn model_id(&self) -> &str {
        "stub"
    }
}

fn source_note() -> kgx_core::Note {
    // Minimal source note; only body + rel_path are read by extract.
    kgx_core::Note {
        body: "Alice decided to migrate billing to Iceberg in Dublin.".into(),
        rel_path: std::path::PathBuf::from("raw/2026-07-05-adr.md"),
        fm: kgx_core::Frontmatter {
            r#type: NoteType::Source,
            id: "01STUB".into(),
            title: "adr".into(),
            status: kgx_core::Status::Active,
            valid_from: None,
            valid_to: None,
            recorded_at: None,
            supersedes: vec![],
            superseded_by: None,
            source: None,
            confidence: kgx_core::Confidence::Medium,
            sources_count: 1,
            tags: vec![],
            links: vec![],
            entity_type: None,
            aliases: vec![],
            created_by: Default::default(),
            created_via: Default::default(),
            extra: Default::default(),
        },
    }
}

const POLE_RESPONSE: &str = r#"{"facts":[{
  "title":"Billing migrates to Iceberg",
  "body":"Alice decided the billing ledger migrates to Iceberg.",
  "confidence":"high",
  "entities":[
    {"name":"Alice","entity_type":"person","rel":"decided"},
    {"name":"Apache Iceberg","entity_type":"object"},
    {"name":"Dublin","entity_type":"location","rel":"located_at"},
    "legacy-bare-entity"
  ]}]}"#;

#[tokio::test]
async fn pole_entities_become_typed_entity_notes_and_relations() {
    let provider = StubProvider(POLE_RESPONSE.into());
    let out = extract(&provider, &source_note(), Intensity::Full).await.unwrap();

    let facts: Vec<_> = out.notes.iter().filter(|n| n.fm.r#type == NoteType::Fact).collect();
    let entities: Vec<_> = out.notes.iter().filter(|n| n.fm.r#type == NoteType::Entity).collect();
    assert_eq!(facts.len(), 1);
    // 3 typed entities + 1 bare legacy entity
    assert_eq!(entities.len(), 4);

    let alice = entities.iter().find(|e| e.fm.title == "Alice").unwrap();
    assert_eq!(alice.fm.entity_type.as_deref(), Some(EntityType::Person.as_str()));
    assert!(alice.rel_path.starts_with("notes/entities"));
    assert!(alice.fm.source.as_deref().unwrap().contains("raw/"));

    let bare = entities.iter().find(|e| e.fm.title == "legacy-bare-entity").unwrap();
    assert_eq!(bare.fm.entity_type, None);

    // fact links all 4 entities
    let fact = facts[0];
    for name in ["Alice", "Apache Iceberg", "Dublin", "legacy-bare-entity"] {
        assert!(fact.fm.links.iter().any(|l| l.contains(name)), "fact should link {name}");
    }

    // typed relations recorded for rel != mentions
    let rels = fact.fm.extra.get("relations").expect("relations key");
    let rels: Vec<(String, String)> = rels
        .as_sequence()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m.get("target").unwrap().as_str().unwrap().to_string(),
                m.get("rel").unwrap().as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert!(rels.contains(&("Alice".into(), "decided".into())));
    assert!(rels.contains(&("Dublin".into(), "located_at".into())));
    assert_eq!(rels.len(), 2, "mentions/untyped rels are not recorded");
}

#[tokio::test]
async fn legacy_string_entities_still_work() {
    let provider = StubProvider(
        r#"{"facts":[{"title":"T","body":"B","confidence":"medium","entities":["alpha","beta"]}]}"#.into(),
    );
    let out = extract(&provider, &source_note(), Intensity::Full).await.unwrap();
    assert_eq!(out.notes.iter().filter(|n| n.fm.r#type == NoteType::Fact).count(), 1);
    assert_eq!(out.notes.iter().filter(|n| n.fm.r#type == NoteType::Entity).count(), 2);
}
```

Add `async-trait = "0.1"` to `[dev-dependencies]` too (the stub impl needs it).

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p kgx-extract --test pole`
Expected: FAIL — no entity notes are produced, `relations` missing.

- [ ] **Step 4: Implement in `pipeline.rs`.** Inside the `for f in v["facts"]...` loop, replace the `links` extraction with parsing of both entity shapes, and accumulate entities across facts:

```rust
// before the loop:
use std::collections::BTreeMap;
struct ExtractedEntity {
    name: String,
    entity_type: Option<kgx_core::EntityType>,
}
let mut entity_pool: BTreeMap<String, ExtractedEntity> = BTreeMap::new(); // key: slug

// inside the loop, replacing the old `links` block:
let mut links: Vec<String> = Vec::new();
let mut relations: Vec<(String, String)> = Vec::new(); // (target name, rel)
for e in f["entities"].as_array().cloned().unwrap_or_default() {
    let (name, etype, rel) = if let Some(s) = e.as_str() {
        (s.to_string(), None, None)
    } else {
        let name = e["name"].as_str().unwrap_or("").to_string();
        let etype = e["entity_type"].as_str().and_then(kgx_core::EntityType::parse);
        let rel = e["rel"].as_str().map(str::to_string);
        (name, etype, rel)
    };
    if name.is_empty() {
        continue;
    }
    links.push(format!("[[{name}]]"));
    if let Some(r) = rel.filter(|r| r != "mentions") {
        relations.push((name.clone(), r));
    }
    entity_pool
        .entry(util::slugify(&name))
        .or_insert(ExtractedEntity { name, entity_type: None })
        .entity_type
        .get_or_insert_with(|| etype.unwrap_or(kgx_core::EntityType::Object));
    // NOTE: only overwrite entity_type when the pool entry has none — see Step 5 refinement.
}
```

**Step 4 refinement (do this instead of the `get_or_insert_with` line above, which types every bare entity):** keep bare entities untyped:

```rust
    let entry = entity_pool
        .entry(util::slugify(&name))
        .or_insert(ExtractedEntity { name, entity_type: None });
    if entry.entity_type.is_none() {
        entry.entity_type = etype;
    }
```

Then attach relations to the fact's frontmatter before pushing the note:

```rust
let mut extra: std::collections::BTreeMap<String, serde_yaml::Value> = Default::default();
if !relations.is_empty() {
    let seq: Vec<serde_yaml::Value> = relations
        .iter()
        .map(|(target, rel)| {
            let mut m = serde_yaml::Mapping::new();
            m.insert("target".into(), serde_yaml::Value::String(target.clone()));
            m.insert("rel".into(), serde_yaml::Value::String(rel.clone()));
            serde_yaml::Value::Mapping(m)
        })
        .collect();
    extra.insert("relations".to_string(), serde_yaml::Value::Sequence(seq));
}
// in the Frontmatter literal: `extra,` instead of `extra: Default::default(),`
```

After the facts loop, emit entity notes:

```rust
for (slug, ent) in entity_pool {
    let id = util::new_ulid();
    notes.push(Note {
        rel_path: PathBuf::from(format!("notes/entities/{slug}.md")),
        body: ent.name.clone(),
        fm: Frontmatter {
            r#type: NoteType::Entity,
            id,
            title: ent.name,
            status: Status::Active,
            valid_from: Some(now[..10].to_string()),
            valid_to: None,
            recorded_at: Some(now.clone()),
            supersedes: vec![],
            superseded_by: None,
            source: Some(source_link.clone()),
            confidence: Confidence::Medium,
            sources_count: 1,
            tags: vec![],
            links: vec![],
            entity_type: ent.entity_type.map(|t| t.as_str().to_string()),
            aliases: vec![],
            created_by: CreatedBy::Agent,
            created_via: CreatedVia::Cli,
            extra: Default::default(),
        },
    });
}
```

(Callers that write extract results to the vault already write each `Note.rel_path`; entity notes land in `notes/entities/`. If the extract CLI dedups/upserts by path, existing entity notes with the same slug are overwritten only via that existing mechanism — check `crates/kgx-cli/src/commands/extract_cmd.rs` and, if it blindly overwrites, skip writing an entity note whose path already exists in the vault: existing human-authored entities win.)

- [ ] **Step 5: Run tests**

Run: `cargo test -p kgx-extract && cargo test --workspace`
Expected: PASS, including both new integration tests and the legacy-format test.

- [ ] **Step 6: Commit**

```bash
git add crates/kgx-extract
git commit -m "feat(ws2): extract classifies POLE entities and typed relations

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 12: WS2 — Brain schema v2: `entity_type` on nodes, typed edges from `relations`

**Files:**
- Modify: `crates/kgx-graph/src/schema.rs`
- Modify: `crates/kgx-graph/src/migrate.rs`
- Modify: `crates/kgx-graph/src/build.rs` (both INSERT INTO notes sites + `derive_edges`)
- Test: `crates/kgx-graph/tests/build_pole.rs`

**Interfaces:**
- Consumes: `Frontmatter.entity_type` (`Option<String>`), `extra["relations"]` (Task 11), `RelType::parse` (Task 10).
- Produces: `notes.entity_type TEXT` column (nullable); `derive_edges` emits typed edges for `relations` entries; `SCHEMA_VERSION = 2`. Task 15 (viz) reads `notes.entity_type`.

- [ ] **Step 1: Write the failing tests** — create `crates/kgx-graph/tests/build_pole.rs`:

```rust
use kgx_core::{Note, RelType};
use kgx_graph::build::derive_edges;

fn note(id: &str, title: &str) -> Note {
    let yaml = format!("type: fact\nid: {id}\ntitle: {title}\n");
    let fm: kgx_core::Frontmatter = serde_yaml::from_str(&yaml).unwrap();
    Note {
        fm,
        body: String::new(),
        rel_path: std::path::PathBuf::from(format!("notes/facts/{title}.md")),
    }
}

#[test]
fn relations_frontmatter_becomes_typed_edges() {
    let mut fact = note("F1", "billing-migrates");
    let alice = note("E1", "Alice");
    let rels: serde_yaml::Value =
        serde_yaml::from_str("- target: Alice\n  rel: decided\n").unwrap();
    fact.fm.extra.insert("relations".into(), rels);

    let edges = derive_edges(&[fact, alice]);
    assert!(
        edges.iter().any(|e| e.src_id == "F1" && e.dst_id == "E1" && e.rel_type == RelType::Decided),
        "expected F1 -decided-> E1, got {edges:?}"
    );
}

#[test]
fn unknown_rel_and_unresolvable_target_are_ignored() {
    let mut fact = note("F1", "t");
    let rels: serde_yaml::Value =
        serde_yaml::from_str("- target: Ghost\n  rel: decided\n- target: t\n  rel: not_a_rel\n").unwrap();
    fact.fm.extra.insert("relations".into(), rels);
    let edges = derive_edges(&[fact]);
    assert!(edges.iter().all(|e| e.rel_type == RelType::LinksTo || e.rel_type != RelType::Decided));
}
```

Note: `serde_yaml` must be in kgx-graph `[dev-dependencies]` (`serde_yaml.workspace = true`) — add if missing.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p kgx-graph --test build_pole`
Expected: FAIL — no `Decided` edge produced.

- [ ] **Step 3: Extend `derive_edges`.** Inside the existing `for n in notes` loop (after the `source` block), add:

```rust
        if let Some(serde_yaml::Value::Sequence(rels)) = n.fm.extra.get("relations") {
            for r in rels {
                let target = r.get("target").and_then(|v| v.as_str());
                let rel = r.get("rel").and_then(|v| v.as_str()).and_then(RelType::parse);
                if let (Some(t), Some(rt)) = (target, rel) {
                    if let Some(dst) = resolve(t) {
                        if dst != n.fm.id {
                            edges.push(Edge {
                                src_id: n.fm.id.clone(),
                                dst_id: dst,
                                rel_type: rt,
                                valid_from: n.fm.valid_from.clone(),
                                valid_to: n.fm.valid_to.clone(),
                            });
                        }
                    }
                }
            }
        }
```

(`serde_yaml` moves from dev-dep to regular dep of kgx-graph if not already there — check `grep serde_yaml crates/kgx-graph/Cargo.toml`; `kgx-core` already exposes `serde_yaml::Value` in `Frontmatter.extra`, so the type is nameable via `serde_yaml` — add `serde_yaml.workspace = true` to `[dependencies]`.)

- [ ] **Step 4: Schema v2.** In `crates/kgx-graph/src/schema.rs`: set `SCHEMA_VERSION: i32 = 2` and add `entity_type TEXT` to the `notes` DDL:

```sql
CREATE TABLE IF NOT EXISTS notes (
  id TEXT PRIMARY KEY, path TEXT NOT NULL, type TEXT NOT NULL, status TEXT NOT NULL,
  valid_from TEXT, valid_to TEXT, recorded_at TEXT, tags TEXT, raw_text TEXT, embedding BLOB,
  entity_type TEXT);
```

In `crates/kgx-graph/src/migrate.rs::ensure_schema`, after the `current < 1` block, add:

```rust
    if current < 2 {
        // v2: POLE entity_type column. Brain is disposable; ALTER keeps
        // existing brains readable until the next `kg index --full`.
        let _ = conn.execute("ALTER TABLE notes ADD COLUMN entity_type TEXT", []);
        conn.execute(
            "INSERT OR IGNORE INTO schema_version (version, applied_at) VALUES (?1, ?2)",
            rusqlite::params![2, kgx_core::util::now_iso()],
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
    }

    Ok(current.max(SCHEMA_VERSION))
```

(The `let _ =` swallows the duplicate-column error when the table was freshly created from the v2 DDL.)

- [ ] **Step 5: Write the column.** In both INSERT sites in `build.rs` (`build_full` line ~183, `build_incremental` line ~275), extend the column list and params:

`INSERT INTO notes (id,path,type,status,valid_from,valid_to,recorded_at,tags,raw_text,embedding,entity_type)` with an added `?11` bound to `n.fm.entity_type` (an `Option<String>` binds as NULL when None — rusqlite handles `Option` natively). Match the existing params! style at each site.

- [ ] **Step 6: Run tests**

Run: `cargo test -p kgx-graph && cargo test --workspace`
Expected: PASS — including pre-existing brain/build tests (v1 brains migrate via the ALTER path; fresh brains get the v2 DDL).

- [ ] **Step 7: Commit**

```bash
git add crates/kgx-graph
git commit -m "feat(ws2): brain schema v2 — entity_type column + typed edges from relations

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 13: WS4 — Contradiction candidates by embedding similarity, not just tags

**Files:**
- Modify: `crates/kgx-dream/src/passes/mod.rs` (shared `cosine`)
- Modify: `crates/kgx-dream/src/passes/dedup.rs` (use shared `cosine`, delete local copy)
- Modify: `crates/kgx-dream/src/passes/contradiction.rs`
- Test: `crates/kgx-dream/src/passes/contradiction.rs` (inline, pure pairing fn)

**Interfaces:**
- Produces: `pub(crate) fn cosine(a: &[f32], b: &[f32]) -> f32` in `passes/mod.rs`; `pub(crate) fn candidate_pairs(facts: &[&Note], embeddings: &[Vec<f32>], threshold: f32, cap: usize) -> Vec<(usize, usize)>` in `contradiction.rs`. Env knob `KGX_DREAM_MAX_PAIRS` (default 200), `KGX_CONTRADICTION_COSINE` (default 0.80).
- Consumes: `ctx.embedder` (same pattern as `dedup.rs`).

- [ ] **Step 1: Move `cosine`.** Cut the `cosine` fn from `dedup.rs` into `passes/mod.rs` as `pub(crate) fn cosine(...)`; in `dedup.rs` replace calls with `super::cosine(...)` (or `crate::passes::cosine`). Run `cargo test -p kgx-dream` — expected PASS (pure refactor).

- [ ] **Step 2: Write the failing tests** — append to `contradiction.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use kgx_core::Note;

    fn note(id: &str, tags: &[&str], links: &[&str]) -> Note {
        let yaml = format!("type: fact\nid: {id}\ntitle: {id}\n");
        let mut fm: kgx_core::Frontmatter = serde_yaml::from_str(&yaml).unwrap();
        fm.tags = tags.iter().map(|s| s.to_string()).collect();
        fm.links = links.iter().map(|s| format!("[[{s}]]")).collect();
        Note { fm, body: String::new(), rel_path: format!("notes/facts/{id}.md").into() }
    }

    #[test]
    fn disjoint_tags_but_similar_embeddings_are_paired() {
        let a = note("A", &["billing"], &[]);
        let b = note("B", &["finance"], &[]);
        let facts = vec![&a, &b];
        // identical vectors → cosine 1.0
        let emb = vec![vec![1.0, 0.0], vec![1.0, 0.0]];
        let pairs = candidate_pairs(&facts, &emb, 0.80, 200);
        assert_eq!(pairs, vec![(0, 1)]);
    }

    #[test]
    fn shared_entity_link_pairs_even_with_dissimilar_embeddings() {
        let a = note("A", &[], &["alice"]);
        let b = note("B", &[], &["alice"]);
        let facts = vec![&a, &b];
        let emb = vec![vec![1.0, 0.0], vec![0.0, 1.0]]; // orthogonal
        assert_eq!(candidate_pairs(&facts, &emb, 0.80, 200), vec![(0, 1)]);
    }

    #[test]
    fn unrelated_facts_are_not_paired_and_cap_is_respected() {
        let a = note("A", &["x"], &["p"]);
        let b = note("B", &["y"], &["q"]);
        let facts = vec![&a, &b];
        let emb = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        assert!(candidate_pairs(&facts, &emb, 0.80, 200).is_empty());

        let c = note("C", &["t"], &[]);
        let d = note("D", &["t"], &[]);
        let e = note("E", &["t"], &[]);
        let all = vec![&c, &d, &e]; // 3 tag-sharing pairs possible
        let emb3 = vec![vec![1.0, 0.0]; 3];
        assert_eq!(candidate_pairs(&all, &emb3, 0.80, 2).len(), 2, "cap limits pairs");
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p kgx-dream candidate_pairs`
Expected: FAIL — `candidate_pairs` not found. (Add `serde_yaml.workspace = true` to kgx-dream `[dev-dependencies]` if the test needs it.)

- [ ] **Step 4: Implement.** In `contradiction.rs`:

```rust
pub(crate) fn candidate_pairs(
    facts: &[&Note],
    embeddings: &[Vec<f32>],
    threshold: f32,
    cap: usize,
) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    'outer: for i in 0..facts.len() {
        for j in (i + 1)..facts.len() {
            let shared_tag = facts[i].fm.tags.iter().any(|t| facts[j].fm.tags.contains(t));
            let shared_link = facts[i]
                .fm
                .links
                .iter()
                .any(|l| facts[j].fm.links.contains(l));
            let similar = super::cosine(&embeddings[i], &embeddings[j]) >= threshold;
            if shared_tag || shared_link || similar {
                pairs.push((i, j));
                if pairs.len() >= cap {
                    break 'outer;
                }
            }
        }
    }
    pairs
}

fn env_f32(key: &str, default: f32) -> f32 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}
```

Rewrite `run()`'s pairing: embed all fact bodies once (mirroring `dedup.rs`), compute pairs, then loop over `pairs` doing the existing LLM classification for each `(a, b) = (facts[i], facts[j])`:

```rust
    if facts.is_empty() {
        return Ok(vec![]);
    }
    let bodies: Vec<String> = facts.iter().map(|n| n.body.clone()).collect();
    let embeddings = ctx.embedder.embed(&bodies)?;
    let threshold = env_f32("KGX_CONTRADICTION_COSINE", 0.80);
    let cap = env_usize("KGX_DREAM_MAX_PAIRS", 200);
    for (i, j) in candidate_pairs(&facts, &embeddings, threshold, cap) {
        let (a, b) = (facts[i], facts[j]);
        // ... existing LLM call + ProposedDiff push, unchanged ...
    }
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p kgx-dream && cargo test --workspace`
Expected: PASS. (With `MockEmbedder`, similarity is noise below threshold for distinct texts; the tag-overlap path preserves prior behavior, so existing dream smoke tests hold.)

- [ ] **Step 6: Commit**

```bash
git add crates/kgx-dream
git commit -m "feat(ws4): contradiction candidates via embedding similarity + shared links, capped

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 14: WS3 — Cytoscape interactive HTML export (POLE-aware)

**Files:**
- Create: `crates/kgx-viz/assets/cytoscape.min.js` (vendored)
- Create: `crates/kgx-viz/src/cytoscape.rs`
- Modify: `crates/kgx-viz/src/model.rs` (`VizNode.entity_type`)
- Modify: `crates/kgx-viz/src/lib.rs` (`pub mod cytoscape;`)
- Test: `crates/kgx-viz/src/cytoscape.rs` (inline)

**Interfaces:**
- Consumes: `notes.entity_type` column (Task 12).
- Produces: `VizNode.entity_type: Option<String>`; `pub fn render(model: &GraphModel) -> String` in `cytoscape.rs` — a single self-contained HTML string (no CDN). Task 15 wires the CLI.

- [ ] **Step 1: Vendor the asset**

```bash
mkdir -p crates/kgx-viz/assets
curl -L https://unpkg.com/cytoscape@3.30.2/dist/cytoscape.min.js -o crates/kgx-viz/assets/cytoscape.min.js
head -c 200 crates/kgx-viz/assets/cytoscape.min.js   # sanity: JS, not an HTML error page
```

- [ ] **Step 2: Add `entity_type` to the model.** In `model.rs`: add `pub entity_type: Option<String>,` to `VizNode`; extend both SELECTs to `SELECT n.id, n.path, n.type, n.status, COALESCE(p.score,0.0), n.entity_type ...`; in `node_from_row` add `entity_type: r.get(5)?,`.

- [ ] **Step 3: Write the failing test** — create `crates/kgx-viz/src/cytoscape.rs`:

```rust
use crate::model::GraphModel;

#[cfg(test)]
mod tests {
    use crate::model::{GraphModel, VizEdge, VizNode};

    #[test]
    fn render_is_self_contained_and_pole_colored() {
        let model = GraphModel {
            nodes: vec![
                VizNode { id: "E1".into(), title: "Alice".into(), r#type: "entity".into(), status: "active".into(), pagerank: 0.5, entity_type: Some("person".into()) },
                VizNode { id: "F1".into(), title: "fact".into(), r#type: "fact".into(), status: "active".into(), pagerank: 0.1, entity_type: None },
            ],
            edges: vec![VizEdge { src: "F1".into(), dst: "E1".into(), rel: "decided".into() }],
        };
        let html = super::render(&model);
        assert!(html.contains("<html"));
        assert!(html.contains("cytoscape"), "embeds the library");
        assert!(!html.contains("https://unpkg.com"), "no CDN — self-contained");
        assert!(html.contains("\"E1\""));
        assert!(html.contains("person"));
        assert!(html.contains("#e15759"), "POLE person color present");
        assert!(html.contains("decided"), "edge rel label present");
    }
}
```

- [ ] **Step 4: Run test to verify it fails**

Run: `cargo test -p kgx-viz cytoscape`
Expected: FAIL — `render` not found (after adding `pub mod cytoscape;` to lib.rs).

- [ ] **Step 5: Implement `render`**:

```rust
const CYTOSCAPE_JS: &str = include_str!("../assets/cytoscape.min.js");

/// POLE color coding: person red, object blue, location green, event orange.
const POLE_COLORS: &[(&str, &str)] = &[
    ("person", "#e15759"),
    ("object", "#4e79a7"),
    ("location", "#59a14f"),
    ("event", "#f28e2b"),
];

pub fn render(model: &GraphModel) -> String {
    let elements: Vec<serde_json::Value> = model
        .nodes
        .iter()
        .map(|n| {
            serde_json::json!({"data": {
                "id": n.id, "label": n.title, "type": n.r#type,
                "status": n.status, "pagerank": n.pagerank,
                "entity_type": n.entity_type,
            }})
        })
        .chain(model.edges.iter().enumerate().map(|(i, e)| {
            serde_json::json!({"data": {
                "id": format!("e{i}"), "source": e.src, "target": e.dst, "label": e.rel,
            }})
        }))
        .collect();
    let elements_json = serde_json::to_string(&elements).unwrap_or_else(|_| "[]".into());
    let color_rules: String = POLE_COLORS
        .iter()
        .map(|(t, c)| {
            format!(
                "{{ selector: 'node[entity_type = \"{t}\"]', style: {{ 'background-color': '{c}' }} }},"
            )
        })
        .collect();
    format!(
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>KGX graph</title>
<style>
  body {{ margin:0; font:13px system-ui; }}
  #cy {{ position:absolute; top:40px; bottom:0; left:0; right:0; }}
  #bar {{ height:40px; display:flex; gap:8px; align-items:center; padding:0 12px; border-bottom:1px solid #ccc; }}
  #info {{ position:absolute; right:12px; top:52px; width:280px; background:#fff; border:1px solid #ccc; padding:8px; display:none; }}
</style>
<script>{lib}</script>
</head><body>
<div id="bar">
  <strong>KGX graph</strong>
  <label>type <select id="ftype"><option value="">all</option></select></label>
  <span id="counts"></span>
</div>
<div id="cy"></div>
<div id="info"></div>
<script>
const elements = {elements};
const cy = cytoscape({{
  container: document.getElementById('cy'),
  elements: elements,
  layout: {{ name: 'cose', animate: false }},
  style: [
    {{ selector: 'node', style: {{ 'label': 'data(label)', 'font-size': 9, 'width': 'mapData(pagerank, 0, 1, 12, 40)', 'height': 'mapData(pagerank, 0, 1, 12, 40)', 'background-color': '#9aa0a6' }} }},
    {color_rules}
    {{ selector: 'edge', style: {{ 'label': 'data(label)', 'font-size': 7, 'curve-style': 'bezier', 'target-arrow-shape': 'triangle', 'width': 1, 'line-color': '#bbb' }} }},
    {{ selector: 'node[status = "superseded"], node[status = "archived"]', style: {{ 'opacity': 0.35 }} }}
  ]
}});
const types = [...new Set(elements.filter(e => !e.data.source).map(e => e.data.type))];
const sel = document.getElementById('ftype');
types.forEach(t => {{ const o = document.createElement('option'); o.value = t; o.textContent = t; sel.appendChild(o); }});
sel.onchange = () => {{
  cy.nodes().forEach(n => n.style('display', (!sel.value || n.data('type') === sel.value) ? 'element' : 'none'));
}};
cy.on('tap', 'node', evt => {{
  const d = evt.target.data();
  const info = document.getElementById('info');
  info.style.display = 'block';
  info.innerHTML = '<b>' + d.label + '</b><br>type: ' + d.type + (d.entity_type ? ' / ' + d.entity_type : '') + '<br>status: ' + d.status + '<br>pagerank: ' + d.pagerank.toFixed(4) + '<br>id: ' + d.id;
}});
document.getElementById('counts').textContent = cy.nodes().length + ' nodes, ' + cy.edges().length + ' edges';
</script>
</body></html>"#,
        lib = CYTOSCAPE_JS,
        elements = elements_json,
        color_rules = color_rules,
    )
}
```

(`serde_json` is already a kgx-viz dependency.)

- [ ] **Step 6: Run tests**

Run: `cargo test -p kgx-viz`
Expected: PASS (including any existing html/mermaid/dot tests — `VizNode` gained a field; fix their constructors by adding `entity_type: None`).

- [ ] **Step 7: Commit**

```bash
git add crates/kgx-viz
git commit -m "feat(ws3): self-contained Cytoscape HTML export with POLE colors

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 15: WS3 — GraphML export + CLI wiring for both new formats

**Files:**
- Create: `crates/kgx-viz/src/graphml.rs`
- Modify: `crates/kgx-viz/src/lib.rs` (`pub mod graphml;`)
- Modify: `crates/kgx-cli/src/commands/graph.rs`

**Interfaces:**
- Produces: `pub fn render(model: &GraphModel) -> String` (GraphML XML with `type`, `status`, `entity_type` node attrs and `rel` edge attr); `kg graph --format cytoscape|graphml` works; cytoscape output file gets `.html` extension.
- Consumes: `GraphModel` (Task 14 shape).

- [ ] **Step 1: Write the failing test** — create `crates/kgx-viz/src/graphml.rs` with tests first:

```rust
use crate::model::GraphModel;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn render(model: &GraphModel) -> String {
    let mut out = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<graphml xmlns="http://graphml.graphdrawing.org/xmlns">
  <key id="d0" for="node" attr.name="title" attr.type="string"/>
  <key id="d1" for="node" attr.name="type" attr.type="string"/>
  <key id="d2" for="node" attr.name="status" attr.type="string"/>
  <key id="d3" for="node" attr.name="entity_type" attr.type="string"/>
  <key id="d4" for="node" attr.name="pagerank" attr.type="double"/>
  <key id="d5" for="edge" attr.name="rel" attr.type="string"/>
  <graph id="kgx" edgedefault="directed">
"#,
    );
    for n in &model.nodes {
        out.push_str(&format!(
            "    <node id=\"{}\"><data key=\"d0\">{}</data><data key=\"d1\">{}</data><data key=\"d2\">{}</data><data key=\"d3\">{}</data><data key=\"d4\">{}</data></node>\n",
            esc(&n.id),
            esc(&n.title),
            esc(&n.r#type),
            esc(&n.status),
            esc(n.entity_type.as_deref().unwrap_or("")),
            n.pagerank,
        ));
    }
    for (i, e) in model.edges.iter().enumerate() {
        out.push_str(&format!(
            "    <edge id=\"e{i}\" source=\"{}\" target=\"{}\"><data key=\"d5\">{}</data></edge>\n",
            esc(&e.src),
            esc(&e.dst),
            esc(&e.rel),
        ));
    }
    out.push_str("  </graph>\n</graphml>\n");
    out
}

#[cfg(test)]
mod tests {
    use crate::model::{GraphModel, VizEdge, VizNode};

    #[test]
    fn graphml_has_typed_nodes_and_edges() {
        let model = GraphModel {
            nodes: vec![VizNode {
                id: "E1".into(),
                title: "Alice & Bob".into(),
                r#type: "entity".into(),
                status: "active".into(),
                pagerank: 0.5,
                entity_type: Some("person".into()),
            }],
            edges: vec![VizEdge { src: "E1".into(), dst: "E1".into(), rel: "owns".into() }],
        };
        let xml = super::render(&model);
        assert!(xml.starts_with("<?xml"));
        assert!(xml.contains("graphml.graphdrawing.org"));
        assert!(xml.contains("Alice &amp; Bob"), "XML-escapes titles");
        assert!(xml.contains(">person<"));
        assert!(xml.contains(">owns<"));
    }
}
```

- [ ] **Step 2: Run tests** (implementation is written with the test here — verify green)

Run: `cargo test -p kgx-viz graphml`
Expected: PASS. Add `pub mod graphml;` to `lib.rs` first.

- [ ] **Step 3: Wire the CLI.** In `crates/kgx-cli/src/commands/graph.rs`, extend the match and extension logic:

```rust
    let content = match format {
        "html" => html::render(&model),
        "cytoscape" => kgx_viz::cytoscape::render(&model),
        "graphml" => kgx_viz::graphml::render(&model),
        "mermaid" => mermaid::render(&model),
        "dot" => dot::render(&model),
        "obsidian" => obsidian_canvas(&model)?,
        other => anyhow::bail!(
            "unknown graph format: {other} (supported: html, cytoscape, graphml, mermaid, dot, obsidian)"
        ),
    };
    let ext = match format {
        "obsidian" => "canvas",
        "cytoscape" => "html",
        f => f,
    };
```

- [ ] **Step 4: Build and smoke-run**

Run: `cargo build -p kgx-cli && cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-viz crates/kgx-cli/src/commands/graph.rs
git commit -m "feat(ws3): GraphML export; kg graph --format cytoscape|graphml

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 16: WS7 — `kg refine`: dream passes scoped to a subgraph

**Files:**
- Create: `crates/kgx-dream/src/refine.rs`
- Modify: `crates/kgx-dream/src/lib.rs` (add `pub mod refine;` — check existing `pub mod` list with `grep "pub mod" crates/kgx-dream/src/lib.rs` and match)
- Create: `crates/kgx-cli/src/commands/refine_cmd.rs`
- Modify: `crates/kgx-cli/src/cli.rs` (new `Refine` variant)
- Modify: `crates/kgx-cli/src/main.rs` (dispatch)
- Modify: `crates/kgx-cli/src/commands/mod.rs` (register module — match how `dream_cmd` is registered)
- Test: `crates/kgx-dream/src/refine.rs` (inline)

**Interfaces:**
- Produces:
  - `pub struct RefineScope { pub query: Option<String>, pub note_id: Option<String>, pub tag: Option<String>, pub limit: usize }`
  - `pub fn select_scope(notes: &[Note], brain: &Brain, scope: &RefineScope) -> Result<Vec<Note>>` — seeds from query (BM25) / note id / tag, expands 1 hop via `kgx_graph::query::neighbors`, returns owned clones.
  - CLI: `kg refine [QUERY] [--note <id>] [--tag <tag>] [--max-iterations N] [--dry-run]` → stages diffs to `.kg/staged_diffs.json` exactly like `kg dream`.
- Consumes: `kgx_graph::query::{bm25_search, neighbors}` (`bm25_search(brain, q, limit) -> Result<Vec<(String, f32)>>`, `neighbors(brain, id, hops) -> Result<Vec<String>>`), `dream()` runner, `DreamContext`.

- [ ] **Step 1: Write the failing test** — `crates/kgx-dream/src/refine.rs`:

```rust
use kgx_core::{Note, Result};
use kgx_graph::Brain;

pub struct RefineScope {
    pub query: Option<String>,
    pub note_id: Option<String>,
    pub tag: Option<String>,
    pub limit: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(id: &str, tag: Option<&str>) -> Note {
        let yaml = format!("type: fact\nid: {id}\ntitle: {id}\n");
        let mut fm: kgx_core::Frontmatter = serde_yaml::from_str(&yaml).unwrap();
        if let Some(t) = tag {
            fm.tags = vec![t.into()];
        }
        Note { fm, body: String::new(), rel_path: format!("notes/facts/{id}.md").into() }
    }

    #[test]
    fn tag_scope_selects_tagged_notes_without_brain_queries() {
        let notes = vec![note("A", Some("billing")), note("B", None), note("C", Some("billing"))];
        let tmp = tempfile::tempdir().unwrap();
        let brain = Brain::open(&tmp.path().join("brain.sqlite")).unwrap();
        let scope = RefineScope { query: None, note_id: None, tag: Some("billing".into()), limit: 50 };
        let picked = select_scope(&notes, &brain, &scope).unwrap();
        let ids: Vec<&str> = picked.iter().map(|n| n.fm.id.as_str()).collect();
        assert!(ids.contains(&"A") && ids.contains(&"C") && !ids.contains(&"B"));
    }

    #[test]
    fn note_scope_selects_exact_id() {
        let notes = vec![note("A", None), note("B", None)];
        let tmp = tempfile::tempdir().unwrap();
        let brain = Brain::open(&tmp.path().join("brain.sqlite")).unwrap();
        let scope = RefineScope { query: None, note_id: Some("B".into()), tag: None, limit: 50 };
        let picked = select_scope(&notes, &brain, &scope).unwrap();
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].fm.id, "B");
    }

    #[test]
    fn empty_scope_errors() {
        let notes = vec![note("A", None)];
        let tmp = tempfile::tempdir().unwrap();
        let brain = Brain::open(&tmp.path().join("brain.sqlite")).unwrap();
        let scope = RefineScope { query: None, note_id: None, tag: None, limit: 50 };
        assert!(select_scope(&notes, &brain, &scope).is_err());
    }
}
```

Add `tempfile.workspace = true` to kgx-dream `[dev-dependencies]` if missing (serde_yaml too, if Task 13 didn't add it).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p kgx-dream refine`
Expected: FAIL — `select_scope` not found.

- [ ] **Step 3: Implement `select_scope`** in `refine.rs`:

```rust
/// Select the refine target subgraph: seeds (query hits / note id / tag
/// matches) expanded one hop through the brain's edge table.
pub fn select_scope(notes: &[Note], brain: &Brain, scope: &RefineScope) -> Result<Vec<Note>> {
    use std::collections::BTreeSet;

    let mut seed_ids: BTreeSet<String> = BTreeSet::new();

    if let Some(id) = &scope.note_id {
        seed_ids.insert(id.clone());
    }
    if let Some(tag) = &scope.tag {
        for n in notes.iter().filter(|n| n.fm.tags.iter().any(|t| t == tag)) {
            seed_ids.insert(n.fm.id.clone());
        }
    }
    if let Some(q) = &scope.query {
        for (id, _score) in kgx_graph::query::bm25_search(brain, q, scope.limit)? {
            seed_ids.insert(id);
        }
    }
    if scope.note_id.is_none() && scope.tag.is_none() && scope.query.is_none() {
        return Err(kgx_core::KgError::Other(
            "kg refine needs a scope: a query, --note <id>, or --tag <tag>".into(),
        ));
    }
    if seed_ids.is_empty() {
        return Err(kgx_core::KgError::Other(
            "refine scope matched no notes — nothing to refine".into(),
        ));
    }

    // Expand 1 hop (best-effort: an unindexed brain just skips expansion).
    let mut selected = seed_ids.clone();
    for id in &seed_ids {
        if let Ok(neigh) = kgx_graph::query::neighbors(brain, id, 1) {
            selected.extend(neigh);
        }
    }

    Ok(notes.iter().filter(|n| selected.contains(&n.fm.id)).cloned().collect())
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p kgx-dream refine`
Expected: PASS.

- [ ] **Step 5: CLI command** — create `crates/kgx-cli/src/commands/refine_cmd.rs` (mirrors `dream_cmd.rs`, scoped):

```rust
use crate::output::emit;
use kgx_dream::{
    context::DreamContext,
    refine::{select_scope, RefineScope},
    run::{dream, DreamOptions},
    PassId,
};
use kgx_graph::Brain;
use std::time::Instant;

#[allow(clippy::too_many_arguments)]
pub fn run(
    json: bool,
    query: Option<String>,
    note: Option<String>,
    tag: Option<String>,
    max_iterations: u32,
    dry_run: bool,
) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;

    let notes = kgx_vault::scan::scan_vault(&root)?;
    let brain = Brain::open(&root.join(".kg/brain.sqlite"))?;
    let provider = kgx_llm::select::provider_from_env()?;
    let embedder = kgx_llm::select::embedder_from_env();

    let scope = RefineScope { query, note_id: note, tag, limit: 25 };
    let scoped = select_scope(&notes, &brain, &scope)?;
    let scoped_count = scoped.len();

    let ctx = DreamContext {
        notes: &scoped,
        brain: &brain,
        provider: provider.as_ref(),
        embedder: embedder.as_ref(),
    };

    let rt = tokio::runtime::Runtime::new()?;
    let result = rt.block_on(dream(
        &ctx,
        DreamOptions { passes: PassId::all().to_vec(), max_iterations },
    ))?;

    if !dry_run {
        std::fs::create_dir_all(root.join(".kg"))?;
        std::fs::write(
            root.join(".kg/staged_diffs.json"),
            serde_json::to_string_pretty(&result.diffs)?,
        )?;
        crate::git::ensure_branch(&root, "kg/dream").ok();
    }

    let data = serde_json::json!({
        "scoped_notes": scoped_count,
        "staged": result.diffs.len(),
        "iterations": result.iterations,
        "dry_run": dry_run,
    });
    emit("refine", data, json, start, |d| {
        println!(
            "refined {} note(s): staged {} diff(s) — run `kg review` to apply",
            d["scoped_notes"], d["staged"]
        )
    });
    Ok(())
}
```

- [ ] **Step 6: Register the command.** In `crates/kgx-cli/src/cli.rs`, after the `Dream` variant:

```rust
    /// Refine a targeted subgraph: run dream passes scoped to a query/note/tag
    Refine {
        /// Retrieval query selecting the notes to refine
        query: Option<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long, default_value = "1")]
        max_iterations: u32,
        #[arg(long)]
        dry_run: bool,
    },
```

In `main.rs` after the `Commands::Dream` arm:

```rust
        Commands::Refine { query, note, tag, max_iterations, dry_run } =>
            commands::refine_cmd::run(cli.json, query, note, tag, max_iterations, dry_run),
```

Add `pub mod refine_cmd;` to `crates/kgx-cli/src/commands/mod.rs`.

- [ ] **Step 7: Run tests + smoke**

Run: `cargo test --workspace && cargo run -p kgx-cli --bin kg -- refine --help`
Expected: tests PASS; help text shows query/--note/--tag/--max-iterations/--dry-run.

- [ ] **Step 8: Commit**

```bash
git add crates/kgx-dream crates/kgx-cli
git commit -m "feat(ws7): kg refine — dream passes scoped to a query/note/tag subgraph

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 17: Docs sync + final verification

**Files:**
- Modify: `README.md` (semantic-by-default, POLE status, `kg refine`, `kg cron remove`, `kg graph --format cytoscape|graphml`, ZCode row if not done in Task 8)
- Modify: `skills/claude/.claude/skills/kgx/SKILL.md`, `skills/codex/AGENTS.md`, `skills/cursor/.cursor/rules/kgx.mdc`, `skills/opencode/.opencode/skills/kgx/SKILL.md`, `skills/zcode/.zcode/skills/kgx/SKILL.md` (mention `kg refine` and the new graph formats where the other verbs are listed)
- Modify: `AGENTS.md` (same verb list updates)

**Interfaces:**
- Consumes: everything shipped in Tasks 1–16.

- [ ] **Step 1: Update the docs.** In each file above, find the command/verb listing (`grep -n "kg dream\|kg cron\|kg graph" <file>`) and:
  - add `kg refine <query>|--note <id>|--tag <tag>` next to `kg dream` with one line: "targeted dream — same passes, scoped subgraph, same review gate";
  - add `kg cron remove <name>` next to the cron verbs;
  - document `kg graph --format cytoscape|graphml`;
  - state that semantic search is on by default (`KGX_EMBED=off` to disable; first `kg index` downloads the embedding model);
  - replace any "POLE is roadmap/aspirational" language with: extract classifies entities as person/object/location/event and emits typed relations **when a real LLM provider is configured** (`KGX_LLM=mock` still yields untyped output).

- [ ] **Step 2: Keep the parity test honest**

Run: `cargo test -p smoke t_skills_valid`
Expected: PASS (doc edits must not remove the 9 MCP tool names).

- [ ] **Step 3: Full verification**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets 2>&1 | tail -5 && cargo test --workspace && git diff --check`
Expected: fmt clean, clippy no new warnings, all tests PASS, no whitespace errors.

- [ ] **Step 4: Benchmark regression gate (final)**

Run: `cargo build --release --bin kg && python3 bench/gen_corpus.py && python3 bench/bench.py`
Expected: Recall@5 ≥ 0.85, MRR ≥ 0.633, the 5 formerly-zero questions hit. Save the output into `bench/results.json` (the harness does this) and quote the numbers in the final commit message.

- [ ] **Step 5: Commit**

```bash
git add README.md AGENTS.md skills bench/results.json
git commit -m "docs: sync README/skills with shipped refine, POLE, cron remove, graph exports

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```
