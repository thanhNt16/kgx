---
name: skill-composition
description: >-
  Compose multiple skills into reusable workflows — sequential pipelines,
  parallel fan-out, dependency graphs, conditional execution, and composite
  verbs. Use when a task needs multiple skills working together as a unit.
---

# Skill Composition

Build composite workflows from individual skills. Think of it as "functions" made of skills.

## Composition Patterns

### Sequential Pipeline
```mermaid
A --> B --> C
```
Each skill consumes the previous skill's output.

```bash
# Composite: ingest → compile → refine → dream
compose_brain_pipeline() {
  local raw_file=$1
  second-brain-ingest "$raw_file"
  second-brain-compile
  second-brain-refine
  second-brain-dream
}
```

### Parallel Fan-out
```mermaid
A --> B
A --> C
A --> D
```
One input, multiple independent skills run concurrently.

```bash
# Composite: analyze code from multiple angles
compose_code_analysis() {
  local target=$1
  gsd:scan "$target" &
  gsd:map-codebase "$target" &
  gsd:intel "$target" &
  wait
}
```

### Dependency Graph (DAG)
```mermaid
A --> C
B --> C
C --> D
```
Skills A and B run in parallel, C waits for both, D waits for C.

```bash
# Composite: full feature cycle
compose_feature() {
  local spec=$1
  # Phase 1: Parallel exploration
  gsd:scan "$spec" --output /tmp/scan.md &
  gsd:map-codebase "$spec" --output /tmp/map.md &
  wait
  # Phase 2: Depends on both
  gsd:discuss-phase --input /tmp/scan.md --input /tmp/map.md --output /tmp/discuss.md
  # Phase 3: Depends on discuss
  gsd:plan-phase --input /tmp/discuss.md --output /tmp/plan.md
  # Phase 4: Execute
  gsd:execute-phase --input /tmp/plan.md
}
```

### Conditional Branching
```mermaid
A -->|success| B
A -->|failure| C
```
Run different skills based on outcome.

```bash
# Composite: try auto-fix, fall back to manual
compose_review_fix() {
  local pr=$1
  if gsd:code-review "$pr" --output /tmp/review.json; then
    gsd:code-review-fix --input /tmp/review.json
  else
    log_warn "Review failed, manual intervention needed"
    gsd:verify-work --pr "$pr"
  fi
}
```

## Composite Verbs (Recommended)

### `brain-cycle` — Full second-brain maintenance
```bash
brain-cycle() {
  local project_root=${1:-.}
  second-brain-ingest "$project_root"
  second-brain-compile
  second-brain-refine
  second-brain-dream
  second-brain-graph "$project_root" --html
}
```

### `debug-loop` — Investigate → Fix → Verify
```bash
debug-loop() {
  local error=$1
  gstack-openclaw-investigate --error "$error" --output /tmp/investigation.md
  # Agent applies fix based on investigation
  gsd:verify-work --input /tmp/investigation.md
}
```

### `feature-ship` — Discuss → Plan → Execute → Review → Ship
```bash
feature-ship() {
  local idea=$1
  gsd:discuss-phase --idea "$idea" --output /tmp/discuss.md
  gsd:plan-phase --input /tmp/discuss.md --output /tmp/plan.md
  gsd:execute-phase --input /tmp/plan.md
  gsd:code-review --output /tmp/review.md
  gsd:code-review-fix --input /tmp/review.md
  gsd:verify-work
  gsd:ship
}
```

### `perf-tune` — Benchmark → Analyze → Fix → Benchmark
```bash
perf-tune() {
  local target=$1
  benchmark --baseline "$target" --output /tmp/base.json
  gstack-benchmark --input /tmp/base.json --output /tmp/analysis.md
  # Agent applies optimizations
  benchmark --compare /tmp/base.json "$target" --output /tmp/after.json
}
```

### `design-build` — Shotgun → Review → HTML → UI Review
```bash
design-build() {
  local brief=$1
  design-shotgun --brief "$brief" --output /tmp/designs/
  plan-design-review --input /tmp/designs/ --output /tmp/approved.md
  design-html --input /tmp/approved.md --output /tmp/page.html
  gsd:ui-review --input /tmp/page.html
}
```

## Skill Composition CLI

Create a `skill-compose` command:

```bash
#!/usr/bin/env bash
# skill-compose — run composite workflows

COMPOSE_DIR="${SKILL_COMPOSE_DIR:-$HOME/.config/skill-compose}"
mkdir -p "$COMPOSE_DIR"

list_compositions() {
  ls "$COMPOSE_DIR"/*.sh 2>/dev/null | xargs -n1 basename | sed 's/\.sh$//'
}

run_composition() {
  local name=$1; shift
  local file="$COMPOSE_DIR/$name.sh"
  [[ -f "$file" ]] || { echo "Unknown composition: $name" >&2; exit 1; }
  source "$file"
  "compose_$name" "$@"
}

case ${1:-} in
  list) list_compositions ;;
  *) run_composition "$@" ;;
esac
```

## Defining Compositions

Save as `$COMPOSE_DIR/brain-cycle.sh`:

```bash
compose_brain_cycle() {
  local root=${1:-.}
  log_info "Starting brain cycle for $root"
  second-brain-ingest "$root"
  second-brain-compile
  second-brain-refine
  second-brain-dream
  second-brain-graph "$root" --html --output "$root/.brain/graph.html"
  log_info "Brain cycle complete"
}
```

## Agent Decision Framework

**User asks** → **Composite**

- "Maintain my second brain" → `brain-cycle`
- "Debug this error" → `debug-loop`
- "Build this feature" → `feature-ship`
- "Optimize performance" → `perf-tune`
- "Design and build this UI" → `design-build`
- "Review and fix this PR" → `review-fix`

## Composition Best Practices

1. **Name composes as verbs** — `brain-cycle`, not `brain_pipeline`
2. **Single entry point** — One function per composition
3. **Pass context via files** — Use `--output`/ `--input` flags
4. **Log each step** — Agent sees progress
5. **Handle failures** — Conditional branches for common failures
6. **Document inputs/outputs** — In function header comment
7. **Test independently** — Each skill works alone too

## Advanced: Dynamic Composition

Build workflows at runtime based on context:

```bash
compose_dynamic() {
  local task=$1
  case $task in
    *bug*|*fix*|*error*)
      compose_debug_loop "$task" ;;
    *feature*|*implement*|*add*)
      compose_feature_ship "$task" ;;
    *review*|*pr*|*merge*)
      compose_review_fix "$task" ;;
    *design*|*ui*|*mockup*)
      compose_design_build "$task" ;;
    *brain*|*knowledge*|*memory*)
      compose_brain_cycle "$task" ;;
    *)
      log_error "Unknown task type: $task"
      return 1 ;;
  esac
}
```

## Integration with Agent Skills

When the user says a composite verb, the agent should:
1. Check if a composition exists
2. If yes, run it
3. If no, orchestrate skills manually using `agent-skill-orchestration` patterns
4. Offer to save the manual orchestration as a new composition

```bash
# In agent logic
if has_composition "$user_request"; then
  run_composition "$user_request"
else
  orchestrate_skills_manually "$user_request"
  suggest_save_as_composition "$user_request"
fi
```
