---
name: kgx-recall
description: Retrieve all notes within 1-2 hops of a named entity from the graph. Use when the user says "what's connected to", "show me around", or wants entity context.
---

# kgx:recall

Retrieve a named entity's graph neighborhood (1-2 hops).

```
kg recall --entity "<entity name>"
```

Return the entity's connected notes grouped by distance (1-hop, 2-hop).
