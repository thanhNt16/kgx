#!/usr/bin/env python3
"""
KGX battle-test corpus generator: simulates a 4-engineer data-platform team
over 36 two-week sprints (18 months) — "DataLake 2.0" migration.

Produces a realistic vault with:
  - 8 entities (people + systems) introduced in sprint 1, reused throughout
  - 36 sprint-cadence notes (planning + retro per sprint)
  - 144 daily-ticket facts (4 tickets/sprint, distributed across sprints)
  - 18 ADRs (one every 2 sprints, some superseding earlier ones)
  - 36 experience/lesson notes
  - 12 incidents
  - 6 superseded facts (explicit "we used to believe X, now Y")

This gives a graph with time-decay pressure: an engineer joining at sprint 36
must answer questions whose evidence spans sprints 1-35, mimicking real
"why did we decide X 8 months ago?" lookups.

All notes are frontmatter-valid KGX notes. Output: vault/ tree under out_dir.
Deterministic (seeded RNG) so benchmark is reproducible.
"""
import os, sys, json, random, hashlib
from datetime import date, timedelta

SEED = 20260119
random.seed(SEED)

OUT = sys.argv[1] if len(sys.argv) > 1 else "/tmp/kgx-corpus"
TEAM = ["alice-chen", "bob-martinez", "cara-nguyen", "david-okafor"]
SYSTEMS = {
    "apache-iceberg": "table format for the analytics warehouse",
    "spark": "ETL cluster on EMR",
    "flink": "real-time streaming on Kubernetes",
    "trino": "ad-hoc federated query engine",
    "postgres": "legacy OLTP datastore (billing_ledger)",
    "kafka": "event bus",
    "airflow": "batch orchestrator",
    "s3": "object storage backing Iceberg",
}
SYSTEM_OWNERS = {
    "flink": "cara-nguyen",
}
SPRINT_START = date(2025, 10, 6)  # Sprint 1 = Monday Oct 6 2025
SPRINT_LEN = 14
N_SPRINTS = 36

os.makedirs(OUT, exist_ok=True)
for sub in ["notes/entities", "notes/facts", "notes/decisions",
            "notes/experiences", "notes/incidents", "raw"]:
    os.makedirs(os.path.join(OUT, sub), exist_ok=True)

def sprint_date(n, day_offset=2):
    return (SPRINT_START + timedelta(days=(n - 1) * SPRINT_LEN + day_offset)).isoformat()

def slugify(s):
    # collapse non-alphanumeric runs into single dashes
    out = []
    prev_dash = True
    for c in s.lower():
        if c.isalnum():
            out.append(c)
            prev_dash = False
        elif c in " -_./":
            if not prev_dash:
                out.append("-")
                prev_dash = True
    return "".join(out).strip("-")[:80]

def ulid_for(kind, n):
    """Deterministic pseudo-ULID: 26 char Crockford base32-ish."""
    h = hashlib.sha256(f"{kind}-{n:04d}".encode()).hexdigest().upper()[:26]
    return "01" + h[2:]  # timestamp-ish prefix for sort stability

def write(path, body):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        f.write(body)

# ---- entities (introduced sprint 1, stable across all 36) ----
people_meta = {
    "alice-chen":  ("Staff Engineer", "architecture", "alice.chen@datalake.io"),
    "bob-martinez":("Platform Lead",  "platform",     "bob.martinez@datalake.io"),
    "cara-nguyen": ("Senior SRE",     "reliability",  "cara.nguyen@datalake.io"),
    "david-okafor":("Data Engineer",  "etl",          "david.okafor@datalake.io"),
}
for i, name in enumerate(TEAM, 1):
    role, team, email = people_meta[name]
    eid = ulid_for("person", i)
    fm = f"""---
type: entity
id: {eid}
entity_type: person
title: {name.replace('-', ' ').title()}
status: active
valid_from: {sprint_date(1)}
tags: [team, person, {team}]
links: []
---
# {name.replace('-', ' ').title()} — {role}

{email} · {team} team. Joined sprint 1.
Owns: {('architecture decisions and the Iceberg migration' if role=='Staff Engineer'
        else 'platform reliability and on-call rotation' if role=='Platform Lead'
        else 'SLOs, alerting, and incident response' if role=='Senior SRE'
        else 'Spark/ Airflow ETL pipelines')}.
"""
    write(os.path.join(OUT, f"notes/entities/{name}.md"), fm)

for i, (sys_name, desc) in enumerate(SYSTEMS.items(), 1):
    eid = ulid_for("system", i)
    owner = SYSTEM_OWNERS.get(sys_name)
    links = f'["[[{owner}]]"]' if owner else "[]"
    owner_line = f"\nOwner: {owner.replace('-', ' ').title()}.\n" if owner else ""
    fm = f"""---
type: entity
id: {eid}
entity_type: system
title: {sys_name}
status: active
valid_from: {sprint_date(1)}
tags: [system, infrastructure]
links: {links}
---
# {sys_name}

{desc.capitalize()}.{owner_line}
"""
    write(os.path.join(OUT, f"notes/entities/{sys_name}.md"), fm)

# ---- ADRs (every 2 sprints; some supersede earlier) ----
adr_topics = [
    ("use-apache-iceberg-as-table-format",
     "Adopt Apache Iceberg as the warehouse table format",
     "Chosen over Delta Lake for Flink integration and Apache governance.",
     "alice-chen"),
    ("flink-on-kubernetes",
     "Run Flink on Kubernetes instead of managed Kinesis Analytics",
     "Lower cost, portable, team already operates K8s.",
     "cara-nguyen"),
    ("trino-for-federated-query",
     "Use Trino for federated ad-hoc queries",
     "Avoids moving data; integrates with Iceberg catalog.",
     "alice-chen"),
    ("backfill-billing-ledger-before-cutover",
     "Backfill billing_ledger from Postgres before Iceberg cutover",
     "Billing team depends on the old table; risk of revenue gap.",
     "bob-martinez"),
    ("airflow-for-batch-orchestration",
     "Standardize on Airflow for batch orchestration",
     "Replaces ad-hoc cron + Jenkins. DAGs are reviewable.",
     "david-okafor"),
    ("s3-with-lifecycle-to-glacier",
     "S3 lifecycle: tier to Glacier after 90 days",
     "Cuts storage cost ~40%. Iceberg metadata unaffected.",
     "bob-martinez"),
    ("kafka-with-exactly-once-source",
     "Kafka sources must use exactly-once connectors",
     "Prevents duplicate billing events downstream.",
     "cara-nguyen"),
]
adr_supersedes = {
    # later ADR supersedes an earlier one — forces graph lookups
    6: 5,   # s3 tiering revised
    7: 4,   # billing backfill approach revised
}
for idx in range(1, N_SPRINTS // 2 + 1):
    sprint = idx * 2
    if idx <= len(adr_topics):
        slug, title, rationale, owner = adr_topics[idx - 1]
    else:
        slug = f"adr-{idx}-misc"
        title = f"ADR {idx}: miscellaneous platform decision"
        rationale = "Minor decision recorded for traceability."
        owner = random.choice(TEAM)
    supersedes = adr_supersedes.get(idx)
    aid = ulid_for("adr", idx)
    sup_field = f'[]'
    extra = ""
    if supersedes:
        prev_id = ulid_for("adr", supersedes)
        sup_field = f'["{prev_id}"]'
        extra = f"\n\n**Supersedes [[adr-{supersedes}-{adr_topics[supersedes-1][0]}]]**: {rationale}\n"
    fm = f"""---
type: decision
id: {aid}
title: "{title}"
status: active
valid_from: {sprint_date(sprint)}
decided_by: {owner}
supersedes: {sup_field}
tags: [adr, architecture]
links: ["[[{owner}]]"]
---
# ADR-{idx:02d} (Sprint {sprint}): {title}

{rationale}{extra}
"""
    write(os.path.join(OUT, f"notes/decisions/adr-{idx:02d}-{slug}.md"), fm)

# ---- daily-ticket facts (4 per sprint, distributed) ----
ticket_facts = [
    ("DL-{n}", "Implement Iceberg partition spec for events table",
     "Partitioned by event_date (days). Spec landed in spark-etl.",
     "david-okafor", ["apache-iceberg", "spark"]),
    ("DL-{n}", "Migrate `page_views` job from cron to Airflow DAG",
     "DAG `page_views_daily` replaces legacy cron. Backfill ran clean.",
     "david-okafor", ["airflow"]),
    ("DL-{n}", "Add Trino catalog entry for the new Iceberg warehouse",
     "Catalog `iceberg_warehouse` registered; query latency p95 320ms.",
     "alice-chen", ["trino", "apache-iceberg"]),
    ("DL-{n}", "Tune Flink checkpoint interval to 60s",
     "Was 10s causing backpressure on kafka billing topic. 60s stable.",
     "cara-nguyen", ["flink", "kafka"]),
    ("DL-{n}", "Wire PagerDuty for billing pipeline failures",
     "Alerts on Airflow DAG failure > 2 in 10 min. Routes to platform on-call.",
     "cara-nguyen", ["airflow"]),
    ("DL-{n}", "Backfill billing_ledger 90 days into Iceberg",
     "Backfill job `billing_backfill` completed; row count matched within 0.01%.",
     "bob-martinez", ["postgres", "apache-iceberg"]),
]
fact_counter = 0
superseded_facts = []  # (file, fid) for later invalidation
# facts created in early sprints get superseded in later sprints (history test):
# the first fact of a later sprint supersedes an early fact.
early_to_supersede = [1, 2, 3, 7, 8, 9]
supersede_map = {}  # later_fact_counter -> early_fact_counter
early_to_later = {}  # early_fact_counter -> (later_counter, later_sprint)
for i, early in enumerate(early_to_supersede):
    later_sprint = 20 + i * 2
    later_counter = (later_sprint - 1) * 4 + 1  # first fact of that sprint
    supersede_map[later_counter] = early
    early_to_later[early] = (later_counter, later_sprint)

for sprint in range(1, N_SPRINTS + 1):
    for j in range(4):
        fact_counter += 1
        tid = 100 + fact_counter
        tmpl = ticket_facts[(sprint + j) % len(ticket_facts)]
        _, title_tmpl, body_tmpl, owner, tags = tmpl
        title = f"{title_tmpl.format(n=tid)}"
        body = body_tmpl.format(n=tid)
        fid = ulid_for("fact", fact_counter)
        status = "active"
        valid_to = ""
        sup_field = ""
        # this fact supersedes an earlier one
        if fact_counter in supersede_map:
            early = supersede_map[fact_counter]
            sup_field = f"supersedes: ['{ulid_for('fact', early)}']"
        # this early fact is superseded by a later fact (forward-dated; the
        # later fact's sprint marks when the early one became stale)
        if fact_counter in early_to_later:
            lc, later_sprint = early_to_later[fact_counter]
            status = "superseded"
            valid_to = f"valid_to: {sprint_date(later_sprint)}"
            sup_field = f"superseded_by: {ulid_for('fact', lc)}"
        # wikilinks: owner + system entities mentioned in tags
        body_links = [f"[[{owner}]]"] + [f"[[{t}]]" for t in tags if t in SYSTEMS]
        links_field = ", ".join(f'"[[{w}]]"' for w in [owner] + [t for t in tags if t in SYSTEMS])
        body_with_links = body + "\n\nRelated: " + " ".join(body_links)
        tag_str = ", ".join(tags) + ", fact"
        fm = f"""---
type: fact
id: {fid}
title: "{title}"
status: {status}
valid_from: {sprint_date(sprint)}
{valid_to}
{sup_field}
confidence: high
source: internal
recorded_at: {sprint_date(sprint)}T12:00:00Z
created_by: agent
created_via: cli
tags: [{tag_str}]
links: [{links_field}]
---
# DL-{tid} (Sprint {sprint}): {title}

{body_with_links}
"""
        write(os.path.join(OUT, f"notes/facts/dl-{tid}-{slugify(title)}.md"), fm)
        if status == "superseded":
            superseded_facts.append((fid, title))

# ---- experiences / lessons ----
lessons = [
    ("Flink backpressure is almost always checkpoint interval, not throughput",
     "We spent 3 days tuning parallelism before finding the 10s checkpoint was the cause.",
     "cara-nguyen"),
    ("Always backfill before cutover, even when row counts match in dev",
     "Production had 14 ghost rows from a clock-skewed producer. Backfill caught them.",
     "bob-martinez"),
    ("Airflow DAGs must be reviewed for idempotency",
     "Non-idempotent DAG caused triple-charged invoices on retry. Add `execution_date` guards.",
     "david-okafor"),
    ("Iceberg compaction should run nightly, not on-write",
     "On-write compaction created 40k small files. Nightly compaction keeps file count < 200.",
     "alice-chen"),
    ("Trino memory cluster grows linearly with concurrent users",
     "Cap concurrency at 12 or queries spill to disk and p95 explodes.",
     "alice-chen"),
    ("Kafka consumer lag alerting needs two windows",
     "Single-window alerts flap. Use 1-min and 5-min windows with AND.",
     "cara-nguyen"),
]
for sprint in range(2, N_SPRINTS + 1, 6):
    idx = (sprint // 6 - 1) % len(lessons)
    title, body, owner = lessons[idx]
    lid = ulid_for("exp", sprint)
    fm = f"""---
type: experience
id: {lid}
title: "{title}"
status: active
valid_from: {sprint_date(sprint)}
recorded_at: {sprint_date(sprint)}T12:00:00Z
tags: [lesson, fact]
links: ["[[{owner}]]"]
---
# Lesson (Sprint {sprint}): {title}

{body}
"""
    write(os.path.join(OUT, f"notes/experiences/exp-s{sprint}-{slugify(title)}.md"), fm)

# ---- incidents ----
incidents = [
    ("INC-{n}", "billing_ledger duplication", "sev2",
     "Exactly-once not enabled on Kafka source; 0.3% events duplicated. Fixed by enabling EOS.",
     "cara-nguyen"),
    ("INC-{n}", "Trino OOM during quarterly close", "sev1",
     "12 concurrent analysts exceeded memory. Capped concurrency; added spill config.",
     "alice-chen"),
    ("INC-{n}", "Airflow scheduler deadlock", "sev2",
     "Stray lock from a failed DAG. Restarted scheduler; added zombie detection.",
     "david-okafor"),
]
for sprint in range(4, N_SPRINTS + 1, 9):
    idx = (sprint // 9 - 1) % len(incidents)
    _, title, sev, body, owner = incidents[idx]
    inc_n = 100 + sprint
    iid = ulid_for("inc", sprint)
    fm = f"""---
type: experience
id: {iid}
title: "{title} ({sev})"
status: active
valid_from: {sprint_date(sprint)}
recorded_at: {sprint_date(sprint)}T12:00:00Z
tags: [incident, {sev}, fact]
links: ["[[{owner}]]"]
---
# Incident INC-{inc_n} (Sprint {sprint}, {sev}): {title}

{body}
"""
    write(os.path.join(OUT, f"notes/incidents/inc-{inc_n}-{slugify(title)}.md"), fm)

# ---- sprint cadence (planning + retro) ----
for sprint in range(1, N_SPRINTS + 1):
    cid = ulid_for("cer", sprint)
    focus = random.choice([
        "Iceberg migration", "Flink stabilization", "billing pipeline hardening",
        "Trino performance", "Airflow migration", "cost reduction",
    ])
    fm = f"""---
type: experience
id: {cid}
title: Sprint {sprint} planning
status: active
valid_from: {sprint_date(sprint)}
tags: [ceremony, planning, fact]
links: []
---
# Sprint {sprint} planning — focus: {focus}

Sprint {sprint} ran {sprint_date(sprint)} to {sprint_date(sprint, 11)}. Focus area: {focus}.
"""
    write(os.path.join(OUT, f"notes/facts/sprint-{sprint:02d}-planning.md"), fm)

# ---- gold question set (the benchmark) ----
# Resolve relevant_note_ids by scanning the ACTUAL generated files (robust to
# rotation index math). Each note's frontmatter `id:` is the source of truth.

def scan_note_ids():
    """Return {title_keyword: note_id} for all notes, keyed by searchable substrings."""
    by_id = {}
    import re
    for dirpath, _, fns in os.walk(os.path.join(OUT, "notes")):
        for fn in fns:
            if not fn.endswith(".md"):
                continue
            path = os.path.join(dirpath, fn)
            text = open(path).read()
            m = re.search(r"^id:\s*(\S+)", text, re.M)
            t = re.search(r"^title:\s*(.+)$", text, re.M)
            if not m:
                continue
            nid = m.group(1)
            title = (t.group(1).strip().strip('"') if t else fn).lower()
            by_id[nid] = title
    return by_id

def find_ids(titles_index, *keywords, limit=1):
    """Find note ids whose title contains all keywords."""
    out = []
    for nid, title in titles_index.items():
        if all(k.lower() in title for k in keywords):
            out.append((nid, title))
    return [n for n, _ in out[:limit]]

titles_index = scan_note_ids()
if not titles_index:
    print("WARNING: no notes scanned for gold set", file=sys.stderr)

def g(question, *title_keywords, patterns=None, category="?", sprint=1, limit=1, cohort="v1"):
    return {
        "question": question,
        "relevant_note_ids": find_ids(titles_index, *title_keywords, limit=limit),
        "expected_patterns": patterns or [title_keywords[0]],
        "category": category,
        "evidence_sprint": sprint,
        "cohort": cohort,
    }

gold = [
    g("Why did we choose Apache Iceberg over Delta Lake?", "iceberg", "table", patterns=["flink", "governance"], category="decision-lookup", sprint=2),
    g("Who owns the Flink streaming infrastructure?", "cara", patterns=["cara", "flink"], category="entity-lookup", sprint=1),
    g("What was the billing_ledger backfill decision?", "backfill", "billing", patterns=["backfill"], category="decision-lookup", sprint=8),
    g("What causes Flink backpressure?", "backpressure", patterns=["checkpoint"], category="experience-lookup", sprint=6),
    g("How was the Trino OOM incident resolved?", "trino", "oom", patterns=["concurrency", "spill"], category="incident-lookup", sprint=13),
    g("What is the Iceberg compaction strategy?", "compaction", patterns=["nightly"], category="experience-lookup", sprint=24),
    g("Who decided to use Airflow for batch orchestration?", "airflow", "batch", patterns=["airflow"], category="decision-lookup", sprint=10, limit=2),
    g("What is the Kafka exactly-once policy?", "exactly", patterns=["exactly-once", "billing"], category="decision-lookup", sprint=14),
    g("How is the Postgres billing_ledger risk handled?", "backfill", "billing", patterns=["backfill"], category="decision-lookup", sprint=8),
    g("What did we learn about Airflow DAG idempotency?", "idempotency", patterns=["idempotency"], category="experience-lookup", sprint=12),
    g("Who is the Staff Engineer driving the Iceberg migration?", "alice", patterns=["alice"], category="entity-lookup", sprint=1),
    g("What is the S3 storage lifecycle policy?", "glacier", patterns=["glacier", "lifecycle"], category="decision-lookup", sprint=12),
    g("What is the Trino query latency target?", "trino catalog", patterns=["320"], category="fact-lookup", sprint=1),
    g("What alerting did we add for billing pipeline failures?", "pagerduty", patterns=["pagerduty"], category="fact-lookup", sprint=2),
    g("How was the Kafka consumer lag alerting improved?", "consumer lag", patterns=["window", "flap"], category="experience-lookup", sprint=30),
]
def g2(question, *kw, **kwargs):
    kwargs.setdefault("cohort", "v2")
    return g(question, *kw, **kwargs)

gold += [
    g2("Which storage layer did the lakehouse standardize on?", "iceberg", "table", category="vocab-mismatch", sprint=2),
    g2("How do we page the on-call when invoicing jobs break?", "pagerduty", category="vocab-mismatch", sprint=2),
    g2("What slows down our stream processing jobs?", "backpressure", category="vocab-mismatch", sprint=6),
    g2("How quickly must analyst ad-hoc queries come back?", "trino catalog", category="vocab-mismatch", sprint=1),
    g2("Where do old files get archived to cut costs?", "glacier", category="vocab-mismatch", sprint=12),
    g2("How do we prevent duplicate charges flowing through the event bus?", "exactly", category="vocab-mismatch", sprint=14),
    g2("Who handles paging and incident response?", "cara", category="vocab-mismatch", sprint=1),
    g2("What tool did we standardize scheduled data jobs on?", "airflow", "batch", category="vocab-mismatch", sprint=10),
    g2("Why do we merge small data files overnight?", "compaction", category="vocab-mismatch", sprint=24),
    g2("What guards against double-charging customers when jobs retry?", "idempotency", category="vocab-mismatch", sprint=12),
    g2("Which storage-tiering decision did the Platform Lead record?", "glacier", category="multi-hop", sprint=12),
    g2("What did the Senior SRE learn about stream stability?", "backpressure", category="multi-hop", sprint=6),
    g2("Which incident hit the federated query engine?", "trino", "oom", category="multi-hop", sprint=13),
    g2("What checkpoint tuning touched the event bus topic?", "checkpoint", "60s", category="multi-hop", sprint=1),
    g2("What work moved the page_views job onto the batch orchestrator?", "page_views", category="multi-hop", sprint=1),
    g2("Which ADR protects the legacy OLTP billing table during cutover?", "backfill", "billing", category="multi-hop", sprint=8),
    g2("Who owns the ETL pipelines that feed the warehouse?", "david", category="multi-hop", sprint=1),
    g2("Which decision came from the architecture owner about federated queries?", "trino", "federated", category="multi-hop", sprint=6),
    g2("What alerting routes to the platform on-call rotation?", "pagerduty", category="multi-hop", sprint=2),
    g2("Which partition layout did the ETL engineer land for events?", "partition spec", category="multi-hop", sprint=1),
    g2("What decision replaced the Airflow batch orchestration ADR?", "glacier", category="temporal", sprint=12),
    g2("Which ADR revised the billing backfill approach?", "exactly", category="temporal", sprint=14),
    g2("What is the current partition spec guidance for the events table?", "partition spec", category="temporal", sprint=20, limit=2),
    g2("Which early cron-to-Airflow migration fact is now superseded?", "page_views", category="temporal", sprint=22, limit=2),
    g2("When did the original Trino catalog fact become stale?", "trino catalog", category="temporal", sprint=24, limit=2),
    g2("Who is the Platform Lead?", "bob", category="entity-relation", sprint=1),
    g2("Who owns SLOs and alerting?", "cara", category="entity-relation", sprint=1),
    g2("Who runs the Spark and Airflow ETL pipelines?", "david", category="entity-relation", sprint=1),
    g2("Who decided to tier storage to Glacier?", "glacier", category="entity-relation", sprint=12),
    g2("Who introduced the exactly-once Kafka connector policy?", "exactly", category="entity-relation", sprint=14),
]
# drop any gold entry with no resolved ids (keeps the benchmark honest)
gold = [e for e in gold if e["relevant_note_ids"]]

with open(os.path.join(OUT, "gold.json"), "w") as f:
    json.dump(gold, f, indent=2)

# manifest for the bench harness
notes_count = sum(len(files) for _, _, files in os.walk(os.path.join(OUT, "notes")))
manifest = {
    "team_size": 4,
    "sprints": N_SPRINTS,
    "duration_months": 18,
    "notes_total": notes_count,
    "entities": len(TEAM) + len(SYSTEMS),
    "facts": fact_counter,
    "decisions": N_SPRINTS // 2,
    "superseded_facts": len(superseded_facts),
    "gold_questions": len(gold),
    "seed": SEED,
}
with open(os.path.join(OUT, "manifest.json"), "w") as f:
    json.dump(manifest, f, indent=2)

print(json.dumps(manifest, indent=2))
