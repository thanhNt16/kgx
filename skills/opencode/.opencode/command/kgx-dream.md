---
description: Consolidate the vault — dedup, contradiction, supersession, stale archival
---
Run vault consolidation. **You (the agent) do the judgment passes** — the
dedup/contradiction/supersession reasoning is yours, not an external LLM's.
`kg dream` was removed; this skill replaces it.

1. **Surface candidates.** Use the LLM-free `dream_step` MCP tool to find
   structural candidates the harness should act on:
   `dream_step({only:"orphan_repair,staleness,open_questions", max_iterations:1})`.
   Also browse with `query_memory` / `deep_search_memory` for clusters of
   similar notes.
2. **Run the judgment passes yourself** over the vault:
   - **Dedup**: near-duplicate facts (same claim, different wording) → merge
     into one, supersede the others.
   - **Contradiction**: facts that conflict → flag, keep the newer/better-
     sourced one, supersede or archive the other.
   - **Supersession**: a fact that replaces an older one → mark the old
     `status: superseded` with `valid_to:` and `superseded_by:`.
   - **Stale archival**: facts whose `valid_to` has passed, or that reference
     missing sources → `status: archived`.
3. **Write proposed diffs** to `.brain/.kg/staged_diffs.json`. The file MUST be
   a **top-level JSON array** of `ProposedDiff` (not `{"diffs": [...]}`):
   ```json
   [
     {
       "id": "01DIFF00000000000000000001",
       "pass": "supersession",
       "kind": "supersede",
       "rationale": "ADR-001 and the storage doc both state CockroachDB was rejected for cost; merge into one canonical note.",
       "severity": "soft",
       "files": [
         {
           "rel_path": "notes/facts/cockroachdb-rejected-for-cost.md",
           "before": null,
           "after": "---\ntype: fact\nid: 01FACT00000000000000000A\ntitle: \"CockroachDB rejected for cost\"\nstatus: active\nvalid_from: 2026-07-07\nsource: \"[[raw/storage-design]]\"\nconfidence: high\nlinks: [\"[[Postgres]]\", \"[[CockroachDB]]\"]\ncreated_by: agent\ncreated_via: mcp\n---\nCockroachDB was evaluated for multi-region writes in 2026-Q1 but rejected due to cost.\n"
         },
         {
           "rel_path": "notes/facts/cockroachdb-cost-duplicate.md",
           "before": "---\ntype: fact\n...old content...\n---\n",
           "after": "---\ntype: fact\nid: 01FACT00000000000000000B\ntitle: \"CockroachDB rejected for cost\"\nstatus: superseded\nsuperseded_by: 01FACT00000000000000000A\nvalid_to: 2026-07-07\nsource: \"[[raw/adr-001-postgres]]\"\ncreated_by: agent\ncreated_via: mcp\n---\nSuperseded by the canonical CockroachDB-rejected note.\n"
         }
       ]
     }
   ]
   ```
   **Schema (strict — `kg review` validates on deserialize):**
   - `kind` ∈ `merge | supersede | archive | add_link | add_note | resummarize | flag_contradiction`
   - `severity` ∈ `info | soft | scope | hard` (`hard` blocks `--approve all`)
   - `id` must be a 26-char ULID; reuse or mint fresh ones deterministically.
   - Each `files[].after` is the **full new file content** (frontmatter + body),
     written verbatim. Its frontmatter must parse, so use only valid enum values:
     - `type` ∈ `fact | entity | decision | experience | moc | source | question | preference | friction`
     - `status` ∈ `active | deprecated | archived | superseded` (NOT `open`)
     - `created_by` ∈ `human | agent`; `created_via` ∈ `cli | mcp | sync` (NOT `dream`)
   - `before` is the prior content (or `null` for a new note) — used for display.
4. Show the user a summary of the staged diffs and ask whether to apply.
5. On approval, apply them: `kg review --approve all --ponytail-audit` via Bash.
   This patches the note files from `staged_diffs.json` and runs the ponytail
   audit (which flags over-broad rationales).
6. Re-index: `kg index --full` so the brain reflects the consolidated vault.

For a scoped pass (a single topic, note, or tag), narrow step 1–2 to that
subgraph instead of the whole vault — same passes, smaller scope.
