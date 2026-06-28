---
name: agent-skill-orchestration
description: >-
  Orchestrate multiple skills and agents for complex tasks — compose skills
  sequentially, in parallel, with dependencies, and handle skill outputs as
  inputs to other skills. Use when a task needs multiple skills working together.
---

# Agent Skill Orchestration

Compose multiple skills to solve complex tasks. Skills can run sequentially
(output of one feeds the next), in parallel (independent work), or with
explicit dependencies (DAG).

## Orchestration Patterns

### Sequential Pipeline
```mermaid
A --> B --> C
```
Skill A produces output → Skill B consumes → Skill C finalizes.

```bash
# Example: Ingest → Compile → Refine → Dream
second-brain-ingest "meeting notes"
second-brain-compile
second-brain-refine
second-brain-dream
```

### Parallel Fan-out
```mermaid
A --> B
A --> C
A --> D
```
One input, multiple independent skills.

```bash
# Example: Analyze code from multiple angles simultaneously
gsd:scan & gsd:map-codebase & gsd:intel
wait
```

### Dependency Graph (DAG)
```mermaid
A --> C
B --> C
C --> D
```
Skills A and B run in parallel, C waits for both, D waits for C.

```bash
# Phase 1: Parallel
skill-a &
skill-b &
wait

# Phase 2: Depends on A+B
skill-c

# Phase 3: Depends on C
skill-d
```

## Skill Communication

### Via Files (Recommended)
Each skill writes to a known location; next skill reads it.

```bash
# Skill A writes output
skill-a --output /tmp/skill-a-output.json

# Skill B reads it
skill-b --input /tmp/skill-a-output.json
```

### Via Environment Variables
```bash
export SKILL_A_RESULT="/path/to/result"
skill-b  # reads $SKILL_A_RESULT
```

### Via Agent Context (In-Session)
The agent holds intermediate results in memory between skill invocations.

## Common Orchestration Flows

### Code Review Pipeline
```bash
# 1. Scan for issues
gsd:code-review --output /tmp/review.json

# 2. Auto-fix issues
gsd:code-review-fix --input /tmp/review.json

# 3. Verify fixes
gsd:code-review --output /tmp/review2.json
```

### Feature Implementation
```bash
# 1. Explore codebase
gsd:scan --output /tmp/scan.md

# 2. Research implementation
gsd:research-phase --input /tmp/scan.md --output /tmp/research.md

# 3. Plan
gsd:plan-phase --input /tmp/research.md --output /tmp/plan.md

# 4. Execute
gsd:execute-phase --input /tmp/plan.md
```

### Debugging Loop
```bash
# 1. Investigate
gstack-openclaw-investigate --error "$ERROR" --output /tmp/investigation.md

# 2. Hypothesize fix
# (agent reasoning)

# 3. Apply fix
# (edit files)

# 4. Verify
gsd:verify-work --input /tmp/investigation.md
```

## Skill Discovery & Selection

### List Available Skills
```bash
# In agent: use skill tool with name="find-skills"
# Or check skills directory
ls ~/.claude/skills/  # or relevant skills dir
```

### Select Skills for Task
| Task Type | Primary Skills | Support Skills |
|-----------|---------------|----------------|
| New feature | gsd:discuss-phase, gsd:plan-phase, gsd:execute-phase | gsd:scan, gsd:map-codebase |
| Bug fix | gstack-openclaw-investigate, systematic-debugging | gsd:verify-work |
| Code review | gsd:code-review, gsd:code-review-fix | gsd:verify-work |
| Performance | benchmark, gsd:scan | gstack-benchmark |
| Design | design-shotgun, design-html, plan-design-review | gsd:ui-phase |
| Documentation | gsd:docs-update, document-release | second-brain-ingest |

## Advanced: Skill Chaining with Conditions

```bash
# Run skill B only if skill A succeeds
if skill-a; then
  skill-b
else
  echo "Skill A failed, stopping"
  exit 1
fi

# Run skill B only if skill A produced output
if skill-a --output /tmp/out.json && [[ -s /tmp/out.json ]]; then
  skill-b --input /tmp/out.json
fi

# Run skill C after both A and B complete (parallel)
skill-a & pid_a=$!
skill-b & pid_b=$!
wait $pid_a $pid_b
skill-c
```

## Agent Decision Framework

**When user asks** → **Orchestrate**

- "Add feature X" → discuss → plan → execute → verify
- "Fix bug Y" → investigate → hypothesize → implement → verify
- "Review this PR" → code-review → code-review-fix → verify
- "Optimize performance" → scan → benchmark → analyze → fix → benchmark
- "Design UI" → design-shotgun → plan-design-review → design-html → ui-review
- "Update docs" → docs-update → document-release
- "Learn codebase" → map-codebase → intel → recall

## Best Practices

1. **Explicit dependencies** — Declare what each skill needs upfront
2. **Idempotent skills** — Safe to re-run; use `--force` for re-execution
3. **Checkpoint outputs** — Save intermediate results to files
4. **Parallel when independent** — Use `& wait` for fan-out
5. **Fail fast** — Check exit codes between stages
6. **Log orchestration** — Record which skills ran, in what order, with what args