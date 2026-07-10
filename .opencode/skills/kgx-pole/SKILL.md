---
name: kgx-pole
description: Extract a POLE (Person/Object/Location/Event) graph from a captured source. Reusable post-ingest step for documents, URLs, and text.
---

# POLE Graph Extraction

Extract a structured POLE graph from any captured source. POLE = **P**erson /
**O**bject / **L**ocation / **E**vent. This is a reusable post-ingest skill —
run it after `kgx:capture` or `kgx:ingest` has captured a source.

## When to use

- After ingesting a document (PDF, Excel, Word, PPTX) via `kg capture` or `ingest_file`
- After crawling a URL via `ingest_url`
- On any existing source note where you want structured POLE entities

This skill **complements** `kgx:extract` — it focuses on entity + relationship
extraction, while `kgx:extract` focuses on atomic facts. Run either or both.

## Workflow

1. **Identify the source.** Call `get_note` on the captured raw source note
   (or use `query_memory({note_type: "source"})` to list sources).

2. **Read the content** and scan for POLE entities:

   **Persons** — named individuals, roles, organizations-as-actors.
   → `upsert_note({type: "entity", entity_type: "person", title: "<name>", links: []})`

   **Objects** — systems, products, documents, tools, physical objects.
   → `upsert_note({type: "entity", entity_type: "object", title: "<name>", links: []})`

   **Locations** — places, addresses, regions, facilities.
   → `upsert_note({type: "entity", entity_type: "location", title: "<name>", links: []})`

   **Events** — meetings, incidents, deployments, dated occurrences.
   → `upsert_note({type: "entity", entity_type: "event", title: "<name>", links: []})`

3. **Extract typed relationships** between entities. For each relationship, add
   it to the fact note's `links` field AND to the `relations` extra field in
   frontmatter (via the body — `upsert_note` handles this through `links`):

   | Relationship | Meaning | Example |
   |---|---|---|
   | `participates_in` | Person participated in event | Alice → Q3 Meeting |
   | `located_at` | Object/event at a location | Server → Data Center |
   | `owns` | Person/org owns an object | Company → Product |
   | `decided` | Person decided something | Alice → Decision X |
   | `caused` | Event/object caused another | Outage → Revenue loss |
   | `mentions_entity` | Fact mentions an entity (general) | Fact → Entity |

   When creating fact notes, link entities via `links` and add typed relations:

   ```
   upsert_note({
     type: "fact",
     title: "Alice presented Q3 results at the all-hands",
     body: "Alice Chen presented Q3 financial results at the all-hands meeting on 2026-07-10.",
     source: "[[raw/2026-07-10-q3-report]]",
     confidence: "high",
     links: ["[[Alice Chen]]", "[[Q3 All-Hands Meeting]]"]
   })
   ```

   The typed relations are derived by `kg index` from the `links` field and the
   `relations` extra field in frontmatter. To add explicit typed relations, use
   the body markdown with wikilinks and let the agent infer the relationship type
   from context. The brain's `derive_edges` function will create typed edges
   when the frontmatter contains a `relations` extra field with `target` and
   `rel` keys.

4. **Index the graph.** Run `kg index --full` via Bash so the POLE entities and
   typed edges are queryable.

5. **Verify.** Query the POLE graph:
   - `kg query --entity-type person` — list all person entities
   - `kg recall --entity "Alice Chen" --relations` — see Alice's typed relationships
   - Or via MCP: `query_memory({note_type: "entity", entity_type: "person"})`
   - Or via MCP: `recall_entity({entity: "Alice Chen", relations: true})`

## Rules

- One entity per `upsert_note` call — never bundle multiple entities.
- Use the exact entity_type values: `person`, `object`, `location`, `event`.
- Only create entities for explicitly named things — avoid speculation.
- Cite the source note via the `source` field.
- Run `kg index --full` after creating entities so they're searchable.
