# Battle-Test Gaps Remediation — Design

**Date:** 2026-07-05
**Status:** Approved (brainstorm validated with user)
**Source:** `docs/kgx-battletest-report.html` §11 (Honest Gaps), §12 (Verdict)

## Goal

Close all 8 gaps surfaced by the 36-sprint battle test in one coordinated effort,
organized as 8 workstreams. The centerpiece decision — which storage/search/graph
engine serves semantic search, keyword search, and the POLE graph — is resolved:
**evolve the existing embedded SQLite spine** (sqlite-vec + FTS5 + edges/petgraph)
rather than adopting an external or multi-model engine.

## Engine decision (context and rationale)

Requirements gathered: prioritize embedded/in-process; Docker Compose acceptable
for a clearly better option; prefer one engine covering vector + keyword + graph;
portable; explorable with UI tooling.

Candidates considered:

| Candidate | Verdict | Reason |
|---|---|---|
| **B. Evolve SQLite spine** (chosen) | ✅ | The measured failure was missing embeddings, not the engine. Smallest diff, keeps 101 tests and the 79 ms disposable-brain property. Viz via export instead of a live server. |
| A. SurrealDB dual-mode (embedded RocksDB + `kg serve` → Surrealist UI) | Rejected | Best literal fit for "one engine, all 3 + UI," but a full rewrite of the brain layer with real regression risk for a problem the benchmark doesn't attribute to the engine. |
| C. Neo4j single container (+GDS, Browser/Bloom) | Rejected | Best graph UI/algorithms, but JVM server kills embedded-first; every `kg` call would need a daemon. |
| Kuzu | Rejected | Ideal on paper (embedded, Cypher, vector+FTS) but discontinued in 2025; unmaintained. |
| CozoDB | Rejected | Embedded multi-model but stagnant maintenance, no UI. |
| Qdrant + Meilisearch + Neo4j suite | Rejected | Three daemons, three consistency problems; explicitly not wanted. |

Key de-risking fact: the brain is disposable and rebuilt from Markdown in 79 ms,
so schema changes are a re-index, never a data migration. The committed `bench/`
harness (220 notes, 15 gold questions) is the regression gate for all retrieval
changes.

## Workstreams

### WS1 — Semantic search on by default

**Gap:** 5/15 gold questions score 0 with keyword-only; `semantic` feature is
opt-in and `KGX_EMBED` must be set explicitly (`crates/kgx-llm/src/select.rs`).

**Design:**
- Make the `semantic` cargo feature default in `kgx-graph`, `kgx-llm`, `kgx-cli`.
- `embedder_from_env()`: when `KGX_EMBED` is unset, default to fastembed.
  `KGX_EMBED=off` opts out; `mock` and `minilm` (candle feature) unchanged.
- ONNX model (~90 MB) downloads on first index, cached in the user cache dir.
- Download/load failure → fall back to mock embedder with a prominent warning
  at index and search time; `kg status` gains a line showing the active
  embedder + model name so degraded mode is always visible.

**Acceptance:** the 5 zero-recall benchmark questions become hits;
Recall@5 ≥ 0.85 on the 15-question set; MRR does not regress.

### WS2 — POLE taxonomy in extract + schema

**Gap:** extract hardcodes `NoteType::Fact` (`crates/kgx-extract/src/pipeline.rs:74`);
`entity_type` is always `None`; no typed relations.

**Design:**
- `kgx-core`: add `EntityType` enum — `Person | Object | Location | Event`.
- Extract prompt (`prompt.rs`): instruct the LLM to (a) classify each entity
  into POLE, (b) emit typed relations between entities and facts
  (e.g. `participates_in`, `located_at`, `owns`, `decided`, `caused`).
- `pipeline.rs`: branch on response — upsert entity notes with `entity_type`
  set, create typed edges alongside atomic fact notes.
- Brain schema: `entity_type` column on nodes, `rel_type` column on edges.
- Conventions preserved: one fact per note, `source: [[raw/...]]` provenance,
  supersede-never-delete.

**Constraint (documented, not hidden):** with `KGX_LLM=mock` there is no POLE
classification — a real provider is required. The contract test (see Testing)
pins the expected response shape independent of any live model.

### WS3 — Graph visualization export

**Gap:** no way to see the actual graph; "explore with UI tools" requirement.

**Design:** extend `kgx-viz` (tera templates already in place):
- `kg graph export --format cytoscape|graphml|mermaid|dot [--out <path>]`.
- `cytoscape`: a **self-contained interactive HTML file** (Cytoscape.js
  embedded, no CDN) — POLE types color-coded, filters by note type / tag /
  time range, node click shows note content + provenance.
- `graphml`: escape hatch into Gephi / yEd / Neo4j Browser for heavier
  exploration.
- `mermaid`/`dot`: small-subgraph embeds for docs and PRs.

### WS4 — Contradiction detection beyond tags

**Gap:** contradiction/supersession passes only pair facts with overlapping
tags (`crates/kgx-dream/src/passes/*.rs`); disjoint-tag contradictions are
invisible.

**Design:**
- Candidate pairing becomes: embedding cosine similarity ≥ threshold
  (default 0.80, configurable) **or** the two facts share a typed entity edge
  (from WS2).
- Hard cap on candidate pairs per run (configurable) to bound LLM cost.
- Depends on WS1 (real embeddings); benefits from WS2 (entity edges).
- Severity gating unchanged: `Severity::Hard` contradictions still cannot be
  auto-applied.

### WS5 — Cron: macOS calendar parity + `remove`

**Gap:** `render_launchd` (`crates/kgx-cron/src/unit.rs`) only translates
`HH:MM`; other syntax silently emits malformed plists (`Hour: "*-*-* *"`).
No way to delete unit files (`crates/kgx-cli/src/commands/cron.rs`).

**Design:**
- Parse a defined subset of systemd calendar syntax and translate to correct
  `StartCalendarInterval` structures (arrays where needed):
  `hourly`, `daily`, `weekly`, `*-*-* HH:MM:SS`, and day-of-week forms
  (e.g. `Mon *-*-* HH:MM:SS`).
- Anything outside the subset → **hard error** listing the supported forms.
  A malformed plist is never written.
- `kg cron remove <name>`: unload (launchd) / disable (systemd), then delete
  the unit/plist files. `disable` keeps files; `remove` deletes. Missing unit
  → clear error with a hint to `kg cron list`.

### WS6 — `review --reject` and minimal interactive mode

**Gap:** `--reject` parsed into `_reject` and ignored; `--interactive` is a
stub (`crates/kgx-cli/src/commands/review.rs`).

**Design:**
- `--reject <ids|all>`: mark targeted staged diffs rejected in
  `.kg/staged_diffs.json`, exclude from apply; when all diffs are resolved,
  clean up the `kg/dream` branch.
- `--interactive`: numbered prompt loop, one diff at a time —
  `[a]pprove / [r]eject / [s]kip / [q]uit`. No TUI framework dependency.
- `Severity::Hard` contradictions remain un-auto-approvable.

### WS7 — Ship `kg refine` (targeted dream)

**Gap:** refine/curate exists only in PRD docs; `refine.rs` referenced but
absent. README language is ahead of the code.

**Design:** ship a thin, honest verb that reuses existing machinery:
- `kg refine <query>` | `--note <id>` | `--tag <tag>`: retrieval selects the
  target subgraph; the existing 7 dream passes run scoped to that
  neighborhood; diffs stage to the same `kg review` gate.
- Implementation lives in `kgx-dream/src/refine.rs` (the file the docs
  already reference) as a scoping layer over the pass runner — no new
  pipeline.
- Docs updated so "refine/curate" describes shipped behavior.

### WS8 — Harness parity (Codex, Cursor, Opencode)

**Gap:** `codebase-memory-mcp` registered only for Claude Code; Cursor lacks
the `verify-finished` hook and `kgx-codebase` rule; `dev-install.sh` Opencode
branch hardcodes `/usr/local/bin/kg` and omits `--transport stdio`.

**Design:**
- Register `codebase-memory-mcp` in all 4 harness configs.
- Add Cursor `verify-finished` hook + `kgx-codebase` rule file.
- Fix `dev-install.sh` Opencode branch: derive the binary path, include
  `--transport stdio`, match the in-repo template.
- Extend `tests/smoke/tests/t_skills_valid.rs` with a structural parity
  check: all 4 harness configs must reference the same MCP servers and tool
  names as the source of truth (`crates/kgx-mcp/src/tools/mod.rs`). Config
  drift becomes a test failure, not a battle-test surprise.

## Error handling principles (all workstreams)

1. **Fail loud, never silently degrade into wrong output.** Unsupported input
   → clear error naming what is supported (WS5). Degraded mode → visible
   warning + `kg status` indicator (WS1).
2. **Mutations stay gated.** WS2/WS4/WS7 produce staged proposals reviewed
   via `kg review` on a git branch; nothing bypasses the gate.
3. **Bounded LLM cost.** WS4 pairing and WS7 refine cap candidates per run
   (configurable); a large vault cannot trigger an unbounded LLM bill.

## Testing strategy

- **Benchmark regression gate:** `bench/` runs before/after every workstream.
  WS1 acceptance is quantitative (5 flips, Recall@5 ≥ 0.85, MRR no regress).
- **Cron snapshot tests:** every supported calendar form gets an `insta`
  snapshot of the rendered plist/unit; unsupported forms get error tests.
- **POLE contract test:** fixture LLM response drives `pipeline.rs`; asserts
  typed entities + typed edges land correctly in vault and brain schema.
- **Harness parity test:** structural check in `t_skills_valid.rs`.
- Existing 101 tests stay green; each workstream lands as its own reviewable
  commit series.

## Rollout order

1. **WS1** — smallest change, biggest measured win; WS4 depends on it.
2. **WS5 + WS6 + WS8** — independent; parallelizable.
3. **WS2** — POLE schema + extract.
4. **WS4** — needs WS1 embeddings; benefits from WS2 edges.
5. **WS3** — viz, most valuable once POLE types exist to color-code.
6. **WS7** — refine, last; reuses dream passes WS4 just improved.

## Out of scope

- Engine replacement (SurrealDB/Neo4j) — revisit only if the evolved SQLite
  spine fails a future benchmark at larger corpus scale.
- Full TUI for review (minimal prompt loop only).
- Live graph UI server — viz is export-based HTML in this iteration.
