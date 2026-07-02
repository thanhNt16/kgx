---
name: kgx-extract
description: Extract atomic facts, entities, and decisions from a captured source note. Use after kgx:capture or when the user says "extract from this source".
---

# kgx:extract

Extract atomic facts, entities, and decisions from a source note.

```
kg extract --source <source_note_id> --intensity full
```

Return the list of created note IDs grouped by type (fact, entity, decision).
