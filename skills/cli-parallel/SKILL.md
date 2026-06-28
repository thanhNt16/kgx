---
name: cli-parallel
description: >-
  Parallel execution patterns for CLI — speed up bulk operations using GNU
  parallel, xargs -P, background jobs with wait, and job control. Use when
  processing multiple files, running independent commands, or batching API calls.
---

# CLI Parallel Execution

Run independent work concurrently instead of sequentially. The right tool depends
on the task shape.

## Quick Reference

| Tool | Best For | Syntax |
|------|----------|--------|
| `xargs -P` | Simple commands over stdin lines | `cat files.txt \| xargs -P 8 -I {} process {}` |
| `parallel` | Complex replacements, progress, retries | `parallel -j 8 process :::: files.txt` |
| `&` + `wait` | Few long-running commands | `cmd1 & cmd2 & wait` |
| `make -j` | Dependency-aware builds | `make -j$(nproc)` |

## xargs -P Patterns

```bash
# Process each line from stdin with 8 workers
cat targets.txt | xargs -P 8 -I {} ./process {}

# With explicit args, max 2 args per invocation
ls *.log | xargs -P 4 -n 2 analyze_log

# Null-delimited for safe filename handling
find . -name '*.rs' -print0 | xargs -0 -P 8 rustfmt

# Show progress (GNU xargs)
seq 100 | xargs -P 4 -I {} sh -c 'echo "Processing {}"; sleep 0.1'
```

## GNU parallel Patterns

```bash
# Basic: one job per CPU core
parallel ::: cmd1 cmd2 cmd3

# From file (one arg per line)
parallel -j 8 ./worker :::: jobs.txt

# Multiple input sources (cartesian product)
parallel echo {1} {2} ::: a b c ::: 1 2 3

# With replacement strings
parallel convert {1} {1.}.jpg ::: *.png

# Progress bar + eta
parallel --progress --eta ./process :::: jobs.txt

# Retry failed jobs once
parallel --retry-failed ./flaky_job :::: jobs.txt

# Keep output order same as input
parallel -k ./process :::: jobs.txt

# Limit by load average (not just CPU count)
parallel --load 80% ./process :::: jobs.txt
```

## Background Jobs + wait

```bash
# Fire and forget, then wait for all
for f in *.tar.gz; do
  tar -xzf "$f" &
done
wait

# Capture PIDs for selective waiting
pids=()
for f in *.tar.gz; do
  tar -xzf "$f" &
  pids+=($!)
done
wait "${pids[@]}"

# Timeout a background job
timeout 30s long_running_task & wait $!
```

## Job Control (Advanced)

```bash
# Suspend a job: Ctrl+Z, then:
bg %1        # resume in background
fg %1        # resume in foreground
jobs         # list background jobs
kill %1      # kill job 1
disown %1    # detach from shell (survives logout)
```

## Agent Decision Tree

**User says** → **Use**

- "process all these files" → `xargs -P` or `parallel`
- "run these 3 commands at once" → `cmd1 & cmd2 & cmd3 & wait`
- "speed up this loop" → replace `for` with `parallel` or `xargs -P`
- "retry failed items" → `parallel --retry-failed`
- "keep output order" → `parallel -k` or `xargs` (default)
- "show progress" → `parallel --progress`
- "limit by system load" → `parallel --load 75%`

## Common Pitfalls

| Pitfall | Fix |
|---------|-----|
| Output interleaving | Use `parallel -k` or `xargs` (serializes output) |
| Too many open files | Limit with `-P` / `-j`, use `ulimit -n` |
| Glob expansion in parallel | Quote: `parallel cmd ::: "*.txt"` not `parallel cmd ::: *.txt` |
| Variable scope in subshells | Export vars or pass explicitly: `parallel env VAR=$VAR cmd ::: list` |
| Silent failures | Check exit codes: `parallel --halt now,fail=1` |

## When NOT to Parallelize

- Commands with shared mutable state (DB writes, same output file)
- I/O-bound on single disk (may slow down)
- Tiny tasks where overhead > work (use `-n` to batch)
- Order-dependent operations

## Pro Tips

```bash
# Auto-detect CPU count
JOBS=$(nproc)  # Linux
JOBS=$(sysctl -n hw.ncpu)  # macOS

# Dry run first
parallel --dryrun ./process :::: jobs.txt

# Resume from failure
parallel --joblog log.txt ./process :::: jobs.txt
# Later: parallel --resume --joblog log.txt ./process :::: jobs.txt

# Distribute across SSH hosts
parallel -S host1,host2 ./process :::: jobs.txt
```