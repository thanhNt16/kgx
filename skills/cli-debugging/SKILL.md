---
name: cli-debugging
description: >-
  Systematic CLI debugging — strace, ltrace, lsof, netstat/ss, perf, hyperfine,
  valgrind, coredump analysis, and binary inspection. Use when investigating
  crashes, hangs, performance issues, or unexpected behavior.
---

# CLI Debugging Toolkit

Find root causes fast with the right tool for each symptom.

## Quick Reference

| Symptom | First Tool | Deeper Tools |
|---------|------------|--------------|
| Crash/segfault | `coredumpctl` / `gdb` | `valgrind`, `asan` |
| Hang/freeze | `strace -p PID` | `gdb -p PID`, `perf top` |
| Slow startup | `strace -f -e trace=execve` | `perf record`, `hyperfine` |
| High CPU | `perf top` / `htop` | `perf record -g`, `flamegraph` |
| High memory | `valgrind --tool=massif` | `heaptrack`, `jemalloc` |
| Network issues | `ss -tulpn` / `netstat` | `tcpdump`, `wireshark` |
| File issues | `lsof -p PID` | `strace -e trace=file` |
| Library issues | `ldd`, `strace -e open` | `LD_DEBUG=libs` |

## Process Inspection

```bash
# Attach to running process
strace -p <PID> -f -s 200          # syscalls (follow forks, long strings)
ltrace -p <PID> -f -s 200          # library calls
gdb -p <PID>                       # interactive debugger

# From start
strace -f -o trace.log ./program   # full trace to file
strace -f -e trace=network ./prog  # only network syscalls
strace -f -e trace=file ./prog     # only file syscalls
strace -f -e trace=process ./prog  # only process syscalls
strace -c ./prog                   # syscall summary (counts, time, errors)

# Filter by syscall
strace -e open,read,write ./prog
strace -e connect,accept,sendto,recvfrom ./prog
```

## Performance Profiling

```bash
# CPU profiling (Linux)
perf record -g ./program           # record call graph
perf report                        # interactive report
perf script | flamegraph.pl > svg  # flame graph (needs FlameGraph)

# Quick benchmark
hyperfine './program'              # statistical benchmarking
hyperfine --warmup 3 './a' './b'   # compare two commands

# Continuous profiling
perf top                           # live top-like view
perf stat -e cycles,instructions,cache-misses ./prog

# Memory profiling
valgrind --tool=massif ./prog      # heap profiling
valgrind --tool=memcheck ./prog    # memory errors (leaks, invalid access)
heaptrack ./prog                   # faster heap profiler
```

## Network Debugging

```bash
# Port listeners
ss -tulpn                          # TCP/UDP listeners with PIDs
netstat -tulpn                     # legacy equivalent
lsof -i :8080                      # process on port 8080

# Packet capture
tcpdump -i any -n port 8080        # capture port 8080
tcpdump -i any -w capture.pcap     # save to file for Wireshark

# Connection inspection
ss -tan state established          # established TCP connections
watch -n1 'ss -tan | grep :8080'   # live monitor
```

## File & Filesystem

```bash
# Open files by process
lsof -p <PID>                      # all open files
lsof /path/to/file                 # processes holding file

# Filesystem activity
strace -f -e trace=file ./prog     # all file ops
fatrace                            # system-wide file access (needs root)
inotifywait -m -r /path            # watch for changes
```

## Binary Inspection

```bash
# Dependencies
ldd ./binary                       # dynamic deps
objdump -p ./binary | grep NEEDED  # same

# Symbols
nm -C ./binary                     # demangled symbols
objdump -t ./binary                # symbol table

# Strings
strings ./binary                   # printable strings
strings -a ./binary                # scan whole file

# Disassembly
objdump -d ./binary | less         # full disassembly
objdump -d ./binary -M intel       # Intel syntax
```

## Core Dump Analysis

```bash
# Enable core dumps
ulimit -c unlimited
echo '/tmp/core.%e.%p' | sudo tee /proc/sys/kernel/core_pattern

# Analyze
coredumpctl list                   # list recent crashes
coredumpctl gdb <PID>              # open in gdb
coredumpctl dump <PID> --output=core  # save core file

# Manual gdb
gdb ./binary /tmp/core.xxx
# (gdb) bt                    # backtrace
# (gdb) info locals           # local variables
```

## Sanitizers (Compile-time)

```bash
# AddressSanitizer (memory errors)
gcc -fsanitize=address -g -O1 prog.c

# ThreadSanitizer (data races)
gcc -fsanitize=thread -g -O1 prog.c

# UndefinedBehaviorSanitizer
gcc -fsanitize=undefined -g -O1 prog.c
```

## Environment Debugging

```bash
# Dynamic linker debug
LD_DEBUG=libs ./prog               # library loading
LD_DEBUG=bindings ./prog           # symbol binding

# Python
python -X faulthandler -c 'import sys; sys.exit(1)'
PYTHONTRACEMALLOC=5 python ./prog

# Node
node --inspect ./prog              # Chrome DevTools
NODE_DEBUG=fs,net ./prog
```

## Agent Decision Tree

**Observed** → **Run**

- "Segmentation fault" → `coredumpctl gdb <PID>`
- "Program hangs" → `strace -p <PID>`
- "Slow startup" → `strace -f -e trace=execve,open ./prog`
- "High CPU" → `perf top` → `perf record -g ./prog`
- "Memory leak" → `valgrind --tool=memcheck ./prog`
- "Connection refused" → `ss -tulpn | grep PORT`
- "File not found" → `strace -f -e trace=open,access ./prog`
- "Library not found" → `ldd ./binary` → `LD_DEBUG=libs ./binary`

## Pro Tips

```bash
# Strace only failed syscalls
strace -f -e trace=open ./prog 2>&1 | grep -E '= -1 E'

# Compare two runs
strace -f -o run1.log ./prog
strace -f -o run2.log ./prog
diff -u run1.log run2.log | less

# Perf with source annotations
perf record -g ./prog
perf annotate --source --stdio-color
```