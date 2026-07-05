---
name: kgx:dream
description: Run dream consolidation (dedup, contradiction, supersession, stale archival)
disable-model-invocation: true
---

Run full dream consolidation.

1. Run `kg dream --max-iterations 3` via Bash
2. Show the staged diffs to the user
3. Ask user to approve and run `kg review --approve all --ponytail-audit` via Bash
