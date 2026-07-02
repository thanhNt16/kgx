---
name: kgx-dream
description: Run full dream consolidation (dedup, contradiction, supersession, stale archival) and review staged diffs. Use when the user says "dream", "consolidate", or "clean up the vault".
---

# kgx:dream

Run consolidation, then review and approve staged changes.

```
kg dream --max-iterations 3
kg review --approve all --ponytail-audit
```

Return the dream summary (passes run, diffs approved).
