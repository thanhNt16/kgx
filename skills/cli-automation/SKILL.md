---
name: cli-automation
description: >-
  Reusable shell automation patterns — functions, traps, temp files, logging,
  progress bars, dry-run modes, idempotent operations, and script templates.
  Use when building repeatable CLI workflows or agent-executed automation.
---

# CLI Automation Patterns

Build robust, repeatable automation that agents can execute reliably.

## Shell Function Library

```bash
# Source in scripts: source /path/to/automation.sh

# ─── Logging ──────────────────────────────────────────────────────
LOG_LEVEL="${LOG_LEVEL:-info}"  # debug, info, warn, error
_log_level() { case $1 in debug) echo 0;; info) echo 1;; warn) echo 2;; error) echo 3;; esac; }
log() {
  local level=$1; shift
  [[ $(_log_level "$level") -ge $(_log_level "$LOG_LEVEL") ]] && echo "[$(date -u +%H:%M:%S)] [$level] $*" >&2
}
log_debug() { log debug "$@"; }
log_info()  { log info  "$@"; }
log_warn()  { log warn  "$@"; }
log_error() { log error "$@"; }

# ─── Dry-run Guard ────────────────────────────────────────────────
DRY_RUN="${DRY_RUN:-0}"
run() {
  if [[ "$DRY_RUN" -eq 1 ]]; then
    log_info "[DRY-RUN] $*"
  else
    log_debug "Running: $*"
    eval "$@"
  fi
}

# ─── Idempotent File Write ────────────────────────────────────────
write_if_changed() {
  local file=$1 content=$2
  if [[ -f "$file" ]] && diff -u <(cat "$file") <(echo "$content") >/dev/null; then
    log_debug "Unchanged: $file"
    return 0
  fi
  run mkdir -p "$(dirname "$file")"
  run printf '%s\n' "$content" > "$file"
  log_info "Updated: $file"
}

# ─── Safe Temp Files ──────────────────────────────────────────────
tmpfile() { mktemp "${TMPDIR:-/tmp}/$(basename "$0").XXXXXX"; }
tmpdir()  { mktemp -d "${TMPDIR:-/tmp}/$(basename "$0").XXXXXX"; }
cleanup() { [[ -n "$TMPDIRS" ]] && rm -rf $TMPDIRS; }
trap cleanup EXIT
TMPDIRS=$(tmpdir)

# ─── Progress Bar ─────────────────────────────────────────────────
progress() {
  local current=$1 total=$2 width=40
  local filled=$((current * width / total))
  printf '\r[%*s%*s] %d/%d' "$filled" '' $((width - filled)) '' "$current" "$total"
  [[ $current -eq $total ]] && echo
}

# ─── Retry with Backoff ───────────────────────────────────────────
retry() {
  local max=${1:-3} delay=${2:-2} attempt=1
  shift 2
  while ! run "$@"; do
    [[ $attempt -ge $max ]] && return 1
    log_warn "Attempt $attempt failed, retrying in ${delay}s..."
    sleep $delay
    delay=$((delay * 2))
    ((attempt++))
  done
}

# ─── Lock File (prevent concurrent runs) ──────────────────────────
lock_file() {
  local lock="/var/lock/$(basename "$0").lock"
  exec 9>"$lock" || return 1
  flock -n 9 || { log_error "Another instance running"; return 1; }
}
```

## Script Template

```bash
#!/usr/bin/env bash
# Script: <name>
# Description: <one-liner>
# Usage: <script> [options] <args>

set -euo pipefail
IFS=$'\n\t'

# ─── Config ───────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/automation.sh"  # or embed functions above

# ─── Args ─────────────────────────────────────────────────────────
usage() { cat <<EOF
Usage: $(basename "$0") [options] <target>
Options:
  -d, --dry-run     Show what would be done
  -v, --verbose     Debug logging
  -h, --help        This help
EOF
}
DRY_RUN=0 LOG_LEVEL=info
while [[ $# -gt 0 ]]; do
  case $1 in
    -d|--dry-run) DRY_RUN=1 ;;
    -v|--verbose) LOG_LEVEL=debug ;;
    -h|--help) usage; exit 0 ;;
    --) shift; break ;;
    -*) log_error "Unknown option: $1"; usage; exit 1 ;;
    *) break ;;
  esac
  shift
done
TARGET=${1:-}
[[ -z "$TARGET" ]] && { usage; exit 1; }

# ─── Main ─────────────────────────────────────────────────────────
main() {
  lock_file || exit 1
  log_info "Starting: $TARGET"
  # ... do work ...
  log_info "Complete"
}
main "$@"
```

## Common Patterns

### Idempotent Directory Setup/Idempotent Config Deploy
```bash
deploy_config() {
  local src=$1 dst=$2
  write_if_changed "$dst" "$(cat "$src")"
}
```

### Process All Files with Progress
```bash
files=( *.log )
total=${#files[@]}
for i in "${!files[@]}"; do
  process "${files[i]}"
  progress $((i+1)) $total
done
```

### Parallel with Semaphore
```bash
max_jobs=4
semaphore() { while [[ $(jobs -rp | wc -l) -ge $max_jobs ]]; do sleep 0.1; done; }
for f in *.dat; do
  semaphore
  process "$f" &
done
wait
```

### Atomic File Swap
```bash
atomic_write() {
  local file=$1 tmp
  tmp=$(mktemp "${file}.tmp.XXXXXX")
  cat > "$tmp"
  run mv "$tmp" "$file"
}
# Usage: generate_content | atomic_write /etc/config.yaml
```

### Check Prerequisites
```bash
require_cmd() { command -v "$1" >/dev/null || { log_error "Missing: $1"; exit 1; }; }
require_cmd jq
require_cmd curl
```

### Structured Output for Agents
```bash
output_json() { jq -n --arg status "$1" --arg msg "$2" '{status: $status, message: $msg}'; }
# Usage: output_json success "deployed" > /tmp/result.json
```

## Agent Decision Tree

**Need** → **Pattern**

- "Run safely, show what would happen" → `DRY_RUN=1`
- "Don't run twice at once" → `lock_file`
- "Clean up on exit/error" → `trap cleanup EXIT`
- "Retry flaky command" → `retry 3 2 cmd`
- "Show progress to user" → `progress $i $total`
- "Only write if changed" → `write_if_changed`
- "Log for debugging" → `LOG_LEVEL=debug`
- "Machine-readable result" → `output_json`

## CI/CD Integration

```yaml
# GitHub Actions
- name: Run automation
  env:
    DRY_RUN: ${{ github.event_name == 'pull_request' && '1' || '0' }}
    LOG_LEVEL: debug
  run: ./deploy.sh production
```