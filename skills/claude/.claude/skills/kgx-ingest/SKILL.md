---
name: kgx-ingest
description: Capture source material + extract atomic facts from it. Use when the user says "ingest this", "capture and extract", or provides a document to process into the knowledge graph.
---

# kgx:ingest

Capture raw source, then extract atomic facts/entities/decisions.

```
kg capture --from <file|-> --type <doc|transcript|article|code>
kg extract --source <source_note_id> --intensity full
```

Return the captured source note ID and the count of extracted notes.
