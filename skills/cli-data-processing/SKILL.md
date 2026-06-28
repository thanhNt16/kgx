---
name: cli-data-processing
description: >-
  Structured data on the command line — jq, yq, mlr, csvkit, sqlite3, duckdb,
  awk, sed, cut, paste, join, sort, uniq, datamash. Use when transforming,
  querying, filtering, or analyzing JSON, YAML, CSV, TSV, SQL, or log data.
---

# CLI Data Processing Toolkit

Process structured data without writing scripts. Each tool has a sweet spot.

## Quick Tool Selection

| Format | Query/Transform | Aggregate/Analyze | SQL-like |
|--------|-----------------|-------------------|----------|
| JSON | `jq` | `jq` + `mlr` | `q` / `duckdb` |
| YAML | `yq` | `yq` + `mlr` | `q` |
| CSV/TSV | `mlr` / `csvkit` | `mlr` / `csvsql` | `q` / `duckdb` / `sqlite3` |
| Logs | `awk` / `jq` | `mlr` / `datamash` | `lnav` |
| Mixed | `mlr` | `mlr` | `duckdb` |

## JSON — `jq`

```bash
# Basics
jq '.' file.json                     # pretty print
jq '.key' file.json                  # extract field
jq '.[]' file.json                   # array elements
jq '.[] | .field' file.json          # field from each array element

# Filtering
jq 'select(.status == "ok")' file.json
jq 'map(select(.age > 30))' file.json
jq '.[] | select(.tags | contains(["prod"]))' file.json

# Transform
jq '{name: .first + " " + .last, age}' file.json
jq '[.[] | {id, name: .full_name}]' file.json
jq 'group_by(.category) | map({category: .[0].category, count: length})'

# Aggregation
jq 'length' file.json                # count
jq 'map(.value) | add' file.json     # sum
jq 'map(.value) | max' file.json     # max
jq 'group_by(.type) | map({type: .[0].type, avg: (map(.val) | add/length)})'

# Streaming (large files)
jq -c '.[]' huge.json | jq -s 'map(select(.active))'

# Multiple files
jq -s 'add' file1.json file2.json    # concatenate arrays
jq -s '.[0] * .[1]' a.json b.json    # merge objects
```

## YAML — `yq` (v4+, go-based)

```bash
# Same syntax as jq mostly
yq '.key' file.yaml
yq '.[] | select(.enabled)' file.yaml
yq '.servers[] | select(.port == 8080)' file.yaml

# Modify in place
yq -i '.replicas = 3' deployment.yaml
yq -i 'del(.metadata.creationTimestamp)' file.yaml

# Merge
yq -i 'select(fileIndex == 0) * select(fileIndex == 1)' base.yaml overlay.yaml

# Convert
yq -o=json file.yaml > file.json
yq -o=yaml file.json > file.yaml
```

## CSV/TSV — `mlr` (Miller)

```bash
# Basics
mlr --csv head -n 5 file.csv
mlr --csv cut -f name,email file.csv
mlr --csv filter '$status == "active"' file.csv
mlr --csv put '$full = $first . " " . $last' file.csv

# Aggregation
mlr --csv stats1 -a min,max,mean,sum -f value file.csv
mlr --csv group-by category then stats1 -a sum -f value file.csv

# Sorting
mlr --csv sort -f timestamp file.csv
mlr --csv sort -nr count file.csv

# Reshape
mlr --csv reshape -r "^metric_" -o name,value file.csv  # wide to long
mlr --csv reshape -s name,value file.csv                 # long to wide

# Join
mlr --csv join -j id -f users.csv then cut -f id,name,email,order_id orders.csv

# Format conversion
mlr --csv --json cat file.csv > file.json
mlr --tsv --csv cat file.tsv > file.csv
```

## SQL on Files — `q`, `duckdb`, `sqlite3`

```bash
# q — SQL directly on CSV/TSV
q -H -d ',' "SELECT COUNT(*), category FROM file.csv GROUP BY category"
q -H "SELECT * FROM a.csv JOIN b.csv ON a.id = b.id"

# DuckDB — fast analytical SQL
duckdb -c "SELECT * FROM 'file.csv' LIMIT 5"
duckdb -c "SELECT category, COUNT(*) FROM 'file.csv' GROUP BY category"

# SQLite — persistent, indexes
sqlite3 :memory: ".mode csv" ".import file.csv t" "SELECT * FROM t LIMIT 5"
sqlite3 data.db "CREATE INDEX idx_name ON users(name);"
```

## Logs — `awk`, `lnav`, `datamash`

```bash
# awk — field-based
awk '$3 == "ERROR" {print $1, $2, $4}' app.log
awk -F'[:,]' '{print $1}' log.jsonl
awk '{count[$1]++} END {for (ip in count) print ip, count[ip]}' access.log

# datamash — numeric aggregates
datamash -t, mean 3 < data.csv
datamash -W groupby 1 sum 3 < data.tsv
datamash -H -t, pstdev 2 < data.csv

# lnav — log navigator (interactive)
lnav /var/log/syslog
lnav app.log
```

## Text Processing Classics

```bash
# cut — simple column extraction
cut -d',' -f1,3 file.csv

# paste — merge lines
paste -d',' file1.txt file2.txt
paste -sd',' file.txt

# join — relational join on sorted files
join -t',' -1 1 -2 1 <(sort file1.csv) <(sort file2.csv)

# sort/uniq
sort -t',' -k2,2 file.csv
sort -u file.txt
uniq -c | sort -rn
```

## Pipelines: Common Patterns

```bash
# JSON → CSV
jq -r '(.[0] | keys_unsorted) as $keys | $keys, map([.[$keys[]]])[] | @csv' file.json

# CSV → JSON
mlr --csv --json cat file.csv

# YAML → JSON → CSV
yq -o=json file.yaml | jq -r '(.[0] | keys_unsorted) as $keys | $keys, map([.[$keys[]]])[] | @csv'

# Filter log, extract JSON, analyze
grep 'ERROR' app.log | jq -R 'fromjson?' | jq -s 'group_by(.service) | map({service: .[0].service, count: length})'

# Large file streaming
zcat huge.json.gz | jq -c '.[] | select(.important)' | mlr --ijson --ocsv cat > filtered.csv
```

## Agent Decision Tree

**Input + Task** → **Tool**

- JSON + filter/transform → `jq`
- JSON + SQL → `duckdb` / `q`
- YAML + modify → `yq -i`
- CSV + filter/aggregate → `mlr`
- CSV + SQL → `csvsql` / `duckdb`
- TSV + join → `mlr join` / `join`
- Logs + extract fields → `awk` / `jq -R`
- Logs + interactive → `lnav`
- Any + frequency count → `datamash` / `sort | uniq -c`

## Performance Tips

```bash
# Stream large JSON
jq -c '.[]' huge.json | process_each_line

# DuckDB for analytical queries
duckdb -c "SELECT * FROM 'big.csv' WHERE x > 0"

# Parallel
ls *.json | parallel -j4 'jq -c ".[]" {} | process'

# Single-pass aggregation
jq 'group_by(.k) | map({k: .[0].k, v: map(.v) | add})'
```

## Install Checklist

```bash
# macOS
brew install jq yq miller csvkit duckdb sqlite lnav datamash

# Ubuntu/Debian
apt install jq yq miller csvkit duckdb sqlite3 lnav datamash
```
