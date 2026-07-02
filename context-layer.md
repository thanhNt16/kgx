# PRD — `kgd`: A Portable, Self-Hosted Context Layer

**Status:** Draft for review
**Author:** Harry (drafted with Claude)
**Date:** 2026-07-02
**Inspired by:** Paul Iusztin, "From Harness Lock-In to Portable Context Layer" (Decoding AI, Jun 2026); Tree/Scrabble architecture; MongoDB single-store argument; OKF v0.1.

---

## 1. Problem statement

Everything valuable about working with AI agents — research, notes, conversations, decisions, preferences, domain knowledge — currently lives inside whichever harness happens to be in use. This creates three concrete failure modes:

1. **Cold-start on switch.** Moving from Claude Code to another harness discards months of accumulated context and learned preferences.
2. **Skill hostage.** Business logic coupled to one harness's conventions breaks or silently degrades when ported.
3. **Pricing/access risk.** Plan changes, model gating, or repricing cannot be routed around when switching friction is high.

The existing `.brain/` second-brain pattern (Markdown + wikilinks, per-project) solves durability and portability of the *files*, but has no serving layer: every agent re-derives search from `grep`, there is no hybrid (vector + graph + BM25) recall, no cross-project unified memory, and no ambient write-back from conversations.

## 2. Product vision

A **portable context layer** that any harness plugs into via one MCP config entry and, within ~5 minutes, knows who the user is, what they're working on, what matters, and how they like things done. The user owns 100% of the data; the harness is disposable.

```
┌─────────────────────────────────────────────┐
│  Harness (disposable): Claude Code / Codex  │
│  / Cursor / OpenCode / claude.ai            │
└──────────────────┬──────────────────────────┘
                   │ one MCP config entry
┌──────────────────▼──────────────────────────┐
│  Serving layer: kgd MCP server              │
│  6 core tools + hosted skills               │
└──────────────────┬──────────────────────────┘
┌──────────────────▼──────────────────────────┐
│  Unified memory (owned by user)             │
│  Log:  .brain/ Markdown (canon, on disk)    │
│  View: MongoDB (graph + vector + BM25)      │
└─────────────────────────────────────────────┘
```

## 3. Goals and non-goals

### Goals

- G1. Single-command bring-up: `docker compose up` yields a working memory + MCP server.
- G2. Markdown-plus-wikilinks (`.brain/`, OKF-conformant) remains the **canonical, append-only source of truth**. MongoDB is a **rebuildable materialized view** — destroy the container, re-run `index`, get the same graph.
- G3. Six-tool agent contract (3 read, 3 write) — never raw DB operations.
- G4. Continual learning: conversations are ingested back automatically via a harness hook, incrementally every ~10 turns, finalized on session end.
- G5. Curation loop (`refine`, `dream`) with a mandatory human review gate: AI-proposed changes to canon land on a git branch, never on main.
- G6. Harness swap = one config line change. Verified against at least Claude Code and one other harness.
- G7. RAM discipline: vector/graph indexes exist **only** on the materialized view. The log carries no vector index (target: ~4× RAM reduction vs. indexing both, per the article's measurement).

### Non-goals (v1)

- Multi-user / multi-tenant support. Single user, single memory.
- Managed/cloud deployment. Local-first via Docker; remote is out of scope.
- Replacing the per-project `.brain/` skill for lightweight cases — the wiki path remains valid; `kgd` is the precision-at-scale tier.
- Neo4j. Only justified at 3+ hop traversals or graph-centric business logic; revisit post-v1 as an optional read-only sync target for visualization.
- Fine-tuning or model hosting of any kind.

## 4. Personas and primary user flows

**Persona:** a developer running multiple coding agents daily across several projects, with an existing Obsidian-style second brain, who wants agents to start warm and get smarter with use.

### UF-1 — First-run bootstrap

1. User clones repo, runs `docker compose up -d`.
2. `kgd` scaffolds the `home` project brain (`~/brains/home/.brain/`) if absent: `raw/`, `wiki/`, `index.md`, `brain.log.md`, `.okf/manifest`. Additional projects register via `kgd project add <name>`.
3. User adds one entry to the harness MCP config pointing at `kgd`.
4. User runs the `bootstrap` skill: agent asks 6–8 onboarding questions (role, projects, preferences, style), writes them as typed notes via `ingest_conversation`.
5. **Acceptance:** in a fresh session, "what do you know about me?" returns the onboarding facts with source links, in < 5 minutes from `git clone`.

### UF-2 — Ingest a document or URL

1. User: "Ingest this RFC into memory" (file path or URL).
2. Harness calls `ingest_file` / `ingest_url`.
3. Pipeline: fetch → clean → write verbatim to `raw/` (log) → chunk → LLM extracts POLE+O entities + typed relations → entity resolution against existing nodes → embed → upsert into Mongo view with lineage refs back to the raw file.
4. Tool returns a receipt: N chunks, M new entities, K merged entities, links to created notes.
5. **Acceptance:** every extracted node carries `sources:` provenance resolvable to a raw file; `nl_query_memory` finds the content immediately after ingest.

### UF-3 — Recall during work (the everyday flow)

1. User starts a task: "Have we decided anything about auth token storage?"
2. Harness calls `nl_query_memory` → LLM maps NL to a Mongo hybrid-search + graph query (vector + BM25 fused, then 1–2 hop expansion from top entities).
3. Compressed brief returns to context: decisions first, then facts/experiences, each with note IDs.
4. If NL mapping fails or user needs exact filters ("all decisions tagged `auth` since March"), harness falls back to `query_memory` with structured filters.
5. **Acceptance:** p50 recall latency < 2s for the hybrid path; every claim in the brief cites a note ID.

### UF-4 — Deep research over a large result set

1. User: "Everything we know about the VFD data layer, across all projects."
2. Result set > 50 docs → harness uses `deep_search_memory`.
3. Tool progressively discloses: writes intermediate results into a localized on-the-fly YAML/Markdown wiki under `.brain/wiki-cache/<query-slug>/`, returns the index file.
4. Agent navigates the localized wiki instead of stuffing 50 raw chunks into context — trading latency for cost and fidelity.
5. **Acceptance:** context tokens consumed for a 100-doc question are < 25% of naive chunk-stuffing; the localized wiki is reusable across sessions until invalidated.

### UF-5 — Ambient learning from conversations

1. User works normally; a harness hook fires `ingest_conversation` every ~10 turns (incremental) and at session end (finalize).
2. Finalize runs compile-style judgment: extract durable facts, decisions, experiences, preferences; discard ephemera.
3. New knowledge routes through refine (ADD / UPDATE / MERGE / DEPRECATE) before touching the view; contradictions are surfaced, never silently overwritten.
4. **Acceptance:** a preference stated once ("I prefer table-driven tests in Rust") is applied unprompted in a session a week later on a different harness.

### UF-6 — Dream / compress (nightly)

1. Scheduler (compose `cron` sidecar or systemd timer) triggers the `dream` skill.
2. Dream reads graph stats (counts, staleness, orphans, broken links, low-confidence notes), re-runs refine over recently-touched notes, proposes merges/deprecations/archives, strengthens links, and surfaces gaps.
3. All proposed canon changes are committed to a `dream/<date>` git branch with a digest in `brain.log.md`. **Nothing merges without human review.**
4. Morning: user reviews the branch diff (or asks the agent to walk it), merges or rejects.
5. **Acceptance:** dream never mutates `main`; digest lists every proposed change with rationale; rejected proposals don't recur without new evidence.

### UF-7 — Harness swap

1. User copies the single MCP config entry into a second harness.
2. New harness immediately has the same 6 tools, same memory, same skills (served by the MCP server, not the harness).
3. **Acceptance:** the UF-5 preference test passes on the new harness with zero migration steps.

## 5. Architecture

### 5.1 Components

| Component | Tech | Responsibility |
|---|---|---|
| `kgd-server` | Rust (axum + rmcp), reuses `kg` crates | MCP server: 6 tools, hosted skills, hooks endpoint |
| `kgd-pipeline` | Rust, same binary (`kgd worker`) | Ingest pipeline: clean → chunk → extract → resolve → embed → upsert |
| `mongo` | MongoDB 8 (community) | Materialized view: documents, entities, relations, embedding vectors; text + graph queries; one database per project (`kgd_<project>`) |
| `kgd-index` | Rust in-process HNSW (`usearch`) | Vector search per D-1: memory-mapped per-project index, snapshots persisted to Mongo GridFS, rebuildable from the view |
| `.brain/` volume | Host bind mount | Canonical log: raw captures + wiki notes, OKF-conformant, git-versioned |
| `kgd-dream` | Same binary, `kgd dream`, cron sidecar | Nightly consolidation onto a review branch |
| LLM access | Anthropic API (extraction, NL→query, dream) | Configurable per stage; extraction can later swap to a local model |
| Embeddings | Voyage AI (or configurable) | Chunk + entity embeddings |

### 5.2 Data model — the two snapshots

**Log (disk, no indexes beyond filesystem):** the existing `.brain/` conventions, unchanged. `raw/` is immutable verbatim capture; `wiki/` notes carry frontmatter (`id`, `type`, `status`, `confidence`, `sources`, timestamps) and `[[wikilinks]]` as edges. OKF manifest declares the directory as a conformant knowledge package (only required field: `type`).

**View (MongoDB, fully indexed, fully rebuildable, one database per project per D-2):**

```
collections:
  documents   { _id, path, sha256, ingested_at, source_kind, project }
  chunks      { _id, doc_id, ord, text, embedding[1024], bm25-indexed }
  entities    { _id, name, pole_type, subtypes[], summary, embedding,
                aliases[], confidence, status, sources: [doc_id...],
                note_path }            # ← lineage, never copied data
  relations   { _id, src_entity, dst_entity, predicate, weight,
                sources: [chunk_id...], valid_from, valid_to }   # bi-temporal
  communities { _id, level, member_entities[], summary, embedding }  # Leiden
```

Ontology: **POLE+O** (`Person, Organization, Location, Event, Object`) as a thin enum, with optional subtypes (Object → device, software, document, task, topic, project). Chosen because it's balanced — shallow enough for reliable LLM triplet extraction, deep enough to be useful.

**Index policy (G7):** BM25 (Mongo text) + graph indexes on `chunks`, `entities`, `relations`, `communities`; vector search via the in-process HNSW index (D-1), which stores only vector + `_id` pairs and memory-maps its snapshot. Raw log content is never embedded twice; `documents` stores hashes and paths, not indexed text. Rebuild paths: `kgd index --rebuild` replays the log into a fresh view; `--rebuild-vectors` regenerates the HNSW snapshot from stored embeddings without re-calling the embedding API.

### 5.3 Retrieval algorithm

`nl_query_memory` pipeline: NL question → LLM emits a query plan (semantic terms, BM25 terms, entity anchors, filters) → parallel vector search + BM25 → Reciprocal Rank Fusion → 1–2 hop graph expansion from top-ranked entities (preferences → their source conversations is the archetypal 2-hop) → community summaries injected when the question is thematic → compression pass → cited brief. This is the article's hybrid-plus-graph fusion and matches the existing `kg ask` RRF design; PPR (HippoRAG-style) is a v1.1 upgrade to the expansion step.

### 5.4 Serving layer — tools are the contract

Six tools, mirroring Tree's surface. Descriptions are written for agents (the description **is** the contract):

| Tool | Kind | Behavior |
|---|---|---|
| `nl_query_memory` | read | Default. NL → hybrid + graph query. Returns compressed, cited brief. |
| `query_memory` | read | Deterministic structured filters (type, tag, project, date, status). Fallback when NL fails; used by skills for exact lookups. |
| `deep_search_memory` | read | 50+ doc result sets via progressive disclosure into a localized wiki (UF-4). |
| `ingest_url` | write | Fetch → pipeline. Idempotent on content hash. |
| `ingest_file` | write | Local path → pipeline. Idempotent on content hash. |
| `ingest_conversation` | write | Incremental (~10 turns) + finalize modes; runs compile judgment then refine routing. |

All read tools take an optional `project` parameter (default: active project + `home`, per D-2); cross-project questions are explicit multi-scope calls, never a merged index.

**MCP Resource — `briefing://{project}` (D-5):** a ~1k-token session-start brief (identity, active-project state, top preferences, open threads, recent decisions), loaded by the harness at session start so warm-start doesn't depend on the agent choosing to query. Regenerated by dream nightly and by `ingest_conversation` finalize.

**Skills are hosted on the MCP server** (not scattered per-harness) to keep business logic unfragmented: `bootstrap`, `recall-brief`, `research` (deep research → localized wiki), `refine`, `dream`. Skill notes in the graph carry bi-temporal supersession metadata (`valid_from`/`valid_to`, `supersedes`) so improved skills replace rather than shadow old ones. Harness-side there is only a thin `AGENTS.md` pointing at the server. Curation verbs (`refine`, `dream`) are deliberately **not** exposed as raw tools callable mid-conversation; they run as skills/scheduled jobs with the review gate.

### 5.5 Continual-learning hook

Claude Code hook (`Stop` / turn-count) posts the transcript delta to `kgd`'s `/hooks/conversation` endpoint, which enqueues `ingest_conversation`. Equivalent adapters per harness are thin (one script each) because all logic lives server-side.

## 6. Docker Compose specification

```yaml
# docker-compose.yml
services:
  mongo:
    image: mongodb/mongodb-community-server:8.0-ubi9
    command: ["--replSet", "rs0", "--bind_ip_all"]   # replica set: change streams + txns
    volumes:
      - mongo-data:/data/db
    healthcheck:
      test: ["CMD", "mongosh", "--quiet", "--eval", "db.adminCommand('ping').ok"]
      interval: 10s
      retries: 5
    mem_limit: 2g            # G7: keep RAM honest from day one

  mongo-init:                # one-shot: initiate replica set + create indexes
    image: mongodb/mongodb-community-server:8.0-ubi9
    depends_on: { mongo: { condition: service_healthy } }
    entrypoint: ["mongosh", "--host", "mongo", "/init/indexes.js"]
    volumes:
      - ./deploy/indexes.js:/init/indexes.js:ro
    restart: "no"

  kgd:
    build: .
    command: ["kgd", "serve", "--transport", "http", "--port", "8321"]
    environment:
      MONGO_URI: mongodb://mongo:27017/?replicaSet=rs0   # db per project: kgd_<project>
      BRAIN_ROOT: /brains
      ANTHROPIC_API_KEY: ${ANTHROPIC_API_KEY}
      VOYAGE_API_KEY: ${VOYAGE_API_KEY}
      EXTRACTION_MODEL: claude-haiku-4-5-20251001
      QUERY_PLANNER_MODEL: claude-sonnet-4-6
    ports: ["8321:8321"]
    volumes:
      - ${BRAIN_ROOT:-~/brains}:/brains   # root of per-project .brain/ packages
                                          # /brains/home/.brain, /brains/<proj>/.brain
    depends_on:
      mongo: { condition: service_healthy }

  kgd-worker:                # pipeline consumer (queue in Mongo)
    build: .
    command: ["kgd", "worker"]
    environment: *kgd-env    # (anchor the env block above in the real file)
    volumes:
      - ${BRAIN_ROOT:-~/brains}:/brains
    depends_on: [kgd]

  kgd-dream:                 # nightly consolidation
    build: .
    command: ["kgd", "cron", "--schedule", "0 3 * * *", "--job", "dream"]
    environment: *kgd-env
    volumes:
      - ${BRAIN_ROOT:-~/brains}:/brains
    depends_on: [kgd]

volumes:
  mongo-data:
```

Notes: replica-set-of-one is required for change streams (worker queue) and multi-document transactions (atomic refine). Vector search is handled by the in-process HNSW index (D-1) — no extra service. Each `/brains/<project>/.brain/` stays a git repo on the host so dream branches and human review happen with normal git tooling; projects register via `kgd project add <name>` which creates the Mongo database and HNSW namespace.

## 7. Delivery plan

**Phase 0 — Skeleton (week 1).** Compose stack boots; `kgd serve` speaks MCP over HTTP with stub tools; Claude Code connects and lists tools. *Exit: UF-1 steps 1–3.*

**Phase 1 — Ingest + view (weeks 2–3).** `ingest_file`/`ingest_url` end-to-end: raw capture → chunk → POLE+O extraction → entity resolution → embeddings → Mongo upsert with lineage. `kgd index --rebuild` replays the log. *Exit: UF-2 acceptance; rebuild produces an identical view (hash-stable).*

**Phase 2 — Recall + bench (weeks 3–4).** `nl_query_memory` (planner + RRF + graph expansion), `query_memory` filters. Build `kgd-bench`: frozen corpus, gold set (§11), three-arm runner. *Exit: UF-3 acceptance; first three-arm benchmark run recorded as the baseline trend point.*

**Phase 3 — Continual learning (week 5).** `ingest_conversation` incremental/finalize, Claude Code hook adapter, refine routing (ADD/UPDATE/MERGE/DEPRECATE) with contradiction surfacing. *Exit: UF-5 acceptance.*

**Phase 4 — Deep search + dream (weeks 6–7).** `deep_search_memory` localized wikis; dream job with git-branch review gate; Leiden community summaries. *Exit: UF-4 and UF-6 acceptance.*

**Phase 5 — Portability proof (week 8).** Second-harness adapter; bootstrap skill; `briefing://` resource (D-5); friction-note telemetry (§12.1) switched on; OKF conformance check on `.brain/`. *Exit: UF-7 acceptance; the 5-minute cold-start demo recorded; full `kgd-bench` report published showing Arm C uplift over Arms A and B.*

## 8. Success metrics

- **Cold-start:** fresh harness answers "what do you know about me / project X" correctly in < 5 min from config change.
- **Recall quality:** ≥ 80% correctness on the gold set (§11) with accurate citations, and a clear uplift of Arm C over Arm B on suites S1–S3 (the serving layer must beat grep-over-Markdown, or it isn't earning its complexity).
- **RAM:** steady-state Mongo RSS ≤ 2 GB at 10k documents (validates the index-only-the-view policy).
- **Compounding:** week-over-week, ≥ 1 dream-surfaced connection per week the user rates "useful and I'd forgotten it."
- **Trust:** zero silent overwrites of contradicted knowledge; 100% of dream changes reviewed before merge.

## 9. Risks

| Risk | Mitigation |
|---|---|
| Entity resolution quality (dupes / bad merges poison the graph) | Confidence thresholds; ambiguous merges routed to dream branch for human review instead of auto-applying |
| LLM cost of ambient ingestion | Haiku-class model for extraction; incremental deltas only; batch finalize; per-day budget cap with drop-to-raw fallback (raw is never lost) |
| NL→query planner reliability | `query_memory` deterministic fallback; planner outputs validated against a query schema before execution |
| Log/view drift | View is never written except by pipeline replay; nightly `validate` job diffs view against log hashes |
| In-process HNSW drift/corruption (index diverges from stored embeddings) | Snapshot versioning keyed on view hash; `validate` job compares counts; `--rebuild-vectors` is cheap since embeddings are stored |
| Per-project scoping hides relevant cross-project knowledge | `home` project holds durable cross-cutting knowledge; dream may propose promoting a project note to `home`; multi-scope query is one explicit parameter away |
| Scope creep vs. existing `kg` CLI | `kgd` reuses `kg` crates; CLI verbs remain the offline interface, MCP tools the online one — one core, two frontends |

## 10. Resolved decisions (2026-07-02)

- **D-1 — Vector index: Community MongoDB + in-process HNSW.** A Rust-side HNSW index (`usearch` or `hnsw_rs`) lives inside the `kgd` process; snapshots are persisted to Mongo (GridFS) so restarts are warm, and the index is always rebuildable from the view (`kgd index --rebuild-vectors`). Preserves the single-database story and full local ownership. Consequence: the index is per-project and memory-mapped, which also serves the RAM budget (G7).
- **D-2 — Per-project memory.** Each project owns its own `.brain/` package (log) and its own Mongo database (`kgd_<project>`) plus HNSW index (view). No unified cross-project view in v1. The personal scope — preferences, identity, standing context — is itself just a project: a `home` brain that every session mounts alongside the active project's brain. Cross-project questions are answered by the agent querying multiple project scopes explicitly (`query_memory` takes a `project` parameter), not by a merged index. This keeps blast radius, RAM, and OKF packaging boundaries clean.
- **D-3 — Grimoire retired.** No separate skill packager. Skills live solely on the `kgd` MCP server and travel with the server config. Anything worth keeping from `grim` (bi-temporal supersession semantics, SKILL.md conventions) is folded into `kgd`'s hosted-skill format; the standalone tool is archived.
- **D-4 — Ingest cadence: ~10 turns** incremental, finalize on session end (article default). Cost is measured in Phase 3; the per-day budget cap with drop-to-raw fallback (§9) is the safety valve, not a cadence change.
- **D-5 — `get_briefing` MCP resource: yes.** `kgd` exposes a session-start briefing (identity, active-project state, top preferences, open threads) as an MCP Resource so warm-start is structural rather than dependent on the agent choosing to query. The briefing is regenerated by dream nightly and on finalize, capped at ~1k tokens.

## 11. Evaluation benchmark — `kgd-bench`

The question the benchmark answers: **how much better is an agent with the context layer than the same agent without it?** Every run compares three arms on identical tasks, models, and harness config:

- **Arm A — Baseline:** harness with no memory of any kind (fresh session, no MCP).
- **Arm B — Naive memory:** harness + the raw `.brain/` folder on disk with grep/read access (the status quo second-brain pattern). This arm exists so we can attribute gains to the serving layer specifically, not just to having notes.
- **Arm C — kgd:** full context layer (6 tools + briefing resource + hosted skills).

### 11.1 Corpus and gold set

A **frozen benchmark corpus** is built once in Phase 2 from real history: ~200 wiki notes, ~50 raw captures, ~20 past conversations across 2–3 real projects plus `home`, snapshotted and never mutated (benchmark runs use a copy). From it, a **gold question set** is authored by hand:

| Suite | N | Example | What it isolates |
|---|---|---|---|
| S1 Needle recall | 20 | "What runner do staging deploys use?" | Retrieval precision on atomic facts |
| S2 Decision recall | 10 | "Have we decided anything about auth token storage, and why?" | Decision retrieval + rationale fidelity |
| S3 Multi-hop | 10 | "Which preferences came out of the VFD RFC discussions?" | Graph expansion (entity → source → preference) |
| S4 Cross-session task | 10 | Re-run a task solved months ago (e.g. "set up Leiden clustering in this repo") | Experience-note reuse: does the agent start warm? |
| S5 Preference adherence | 10 | Coding/writing tasks where a stored preference should change the output unprompted | Ambient learning value |
| S6 Cold-start identity | 5 | "What am I working on right now? What matters to me?" | Briefing resource (D-5) |
| S7 Negative controls | 10 | Questions the corpus cannot answer | Hallucination pressure: correct answer is "not in memory" |

### 11.2 Metrics

Per arm, per suite:

- **Answer correctness** — LLM-as-judge against gold answers (independent model, rubric-scored 0–2), with a 20% human spot-check per release; judge/human disagreement > 10% invalidates the run.
- **Citation accuracy** — fraction of claims carrying a note ID that actually supports the claim (checked mechanically for existence, judged for support).
- **Groundedness (S7)** — rate of correctly answering "not in memory" instead of confabulating. Arm C must beat Arm A here, not just tie.
- **Efficiency** — total tokens and wall-clock time to final answer; for S4, number of agent turns/tool calls until first correct action.
- **Cost** — API spend per question, including kgd's internal LLM calls (planner, extraction) so the comparison is honest.

Headline number: **uplift = Arm C correctness − Arm B correctness**, reported per suite. If C doesn't clearly beat B on S1–S3, the serving layer isn't earning its complexity — that's the kill criterion for the hybrid-retrieval investment.

### 11.3 System-level benchmarks

Alongside quality: recall latency p50/p95 (target < 2s / < 5s hybrid path), ingest throughput (docs/min), Mongo + kgd RSS at 1k/10k/50k chunks (G7 line: ≤ 2 GB at 10k docs), HNSW rebuild time, and full log-replay rebuild time.

### 11.4 Cadence and regression tracking

`kgd bench run` executes all arms headlessly and writes a scored report to `bench/results/<date>.json`; a small dashboard note in the `home` brain tracks the trend. Runs happen at every phase exit (Phases 2–5) and on every release thereafter. A score drop > 5% on any suite blocks release. The gold set is versioned; questions are only added, never edited, so trends stay comparable — new capabilities get new suites.

## 12. Evolution and adaptation plan

The context layer must get better *because* it is used — and the mechanism for that is the system observing itself through the same memory it serves.

### 12.1 The system observes its own friction

Every recall miss, failed NL→query plan, tool error, and empty result is written back into the `home` brain as a typed `friction` note (a new note type: what was asked, what was returned, which project scope). Additionally, a lightweight harness signal — the user re-asking a semantically similar question within a session, or manually opening `.brain/` files after a query — is logged as an implicit miss. Nothing extra is required from the user; the telemetry *is* memory.

### 12.2 Dream closes the loop

The nightly dream job gains a **friction pass**: cluster the week's friction notes, and for each cluster propose a concrete fix on the dream branch — a missing note to write, an alias to add for entity resolution, a synonym for the query planner, a skill instruction to amend, or a gold-set question to add to `kgd-bench` so the failure becomes a permanent regression test. The weekly digest leads with the **top-3 friction themes**, so Monday-morning review is "here's what your memory failed at and here's the proposed patch," not a raw changelog. This makes adaptation a weekly rhythm with the same human review gate as every other canon change — fast to adapt, impossible to silently self-modify.

### 12.3 Skills that improve themselves (gated)

Because skills live in the graph with bi-temporal supersession (§5.4), dream can propose skill revisions: when friction notes cluster around a skill ("recall-brief keeps surfacing stale decisions"), dream drafts a superseding skill version on the branch, citing the friction notes as evidence. Merging activates it; the old version stays queryable with `valid_to` set. Skill changes additionally require the relevant `kgd-bench` suite to pass before merge is recommended.

### 12.4 Ontology and structure evolve from data, not upfront design

POLE+O subtypes are not fixed: when extraction repeatedly produces `Object:other` entities that cluster semantically (Leiden communities with high internal similarity and no subtype), dream proposes a new subtype with candidate members. Same for note types — if `friction` proves the pattern, other operational types can earn their way in. Structure follows observed usage.

### 12.5 Roadmap beyond v1

- **v1.1 — Retrieval depth:** HippoRAG-style PPR replaces the naive 1–2 hop expansion; per-suite bench uplift must justify the added latency.
- **v1.2 — Proactive memory:** dream surfaces "you should probably know" items into the briefing resource (upcoming staleness, contradictions pending, connections between the active project and `home`), capped so the briefing stays ≤ 1k tokens.
- **v1.3 — Promotion flows:** dream proposes promoting project-scoped notes with cross-project gravity into `home`, softening D-2's isolation cost without merging indexes.
- **v1.4 — Local model option:** extraction and query planning swappable to a local model; bench arms re-run to quantify the quality/cost trade before it becomes a default.
- **Continuous:** OKF conformance tracked against spec revisions so the `.brain/` packages remain portable to whatever consumes OKF next — the whole point is that the layer outlives any single harness, model, or even `kgd` itself.

### 12.6 Adaptation guardrails

Speed of evolution never overrides the trust invariants: canon mutations only via reviewed branches; the log is append-only forever; every adaptive change (skill revision, subtype, planner synonym) is itself a note with provenance, so the system's own evolution is queryable — "why does recall behave this way now?" has an answer with sources.

## Appendix A — Note types (carried over from second-brain conventions)

`entity | decision | fact | experience | person | glossary | preference` — with `preference` added as a first-class type for the continual-learning loop, since preferences are what make the 5-minute warm start feel personal. All types map onto POLE+O in the view (`decision`/`fact`/`experience` → Event/Object subtypes; `person` → Person; `preference` → Object:preference with an edge to Person:me).

## Appendix B — What we deliberately did differently from the article

1. **The log is Markdown, not a Mongo collection.** The article's append-only log lives in the DB; ours is the `.brain/` filesystem. Same RAM property (un-indexed), better ownership story, git-native review, and it doubles as the Level-2 "LLM wiki" fallback — if `kgd` dies, any agent can still be pointed at the folder.
2. **Curation is gated.** Tree's write tools decide when to persist autonomously; we route all canon mutations through refine + dream branches with mandatory human review, per the existing kg design principle.
3. **Rust, not Python.** Reuse of existing `kg` crates; the serving layer is one static binary, which keeps the compose image small and the MCP server trivially portable.