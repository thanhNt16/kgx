#!/usr/bin/env python3
"""
Large-scale KGX corpus generator for performance benchmarking.

Produces a vault with configurable node/edge counts using templated
content with topic rotations so notes have varied text for BM25/SPLADE.

Usage:
  python3 bench/gen_large_corpus.py --out /tmp/kgx-large --nodes 10000 --edges 15000
"""
import os, sys, json, random, hashlib, argparse, shutil
from datetime import date, timedelta

SEED = 20260705
random.seed(SEED)

TOPICS = [
    "iceberg table format", "flink stream processing", "trino federated query",
    "kafka event bus", "airflow batch orchestration", "spark etl pipeline",
    "postgres billing ledger", "s3 object storage", "glacier archival tier",
    "docker container orchestration", "kubernetes cluster management",
    "prometheus monitoring", "grafana dashboard", "terraform infrastructure",
    "helm chart deployment", "nginx ingress gateway", "redis caching layer",
    "elasticsearch log search", "vault secret management", "gitlab ci pipeline",
]

PERSON_NAMES = [
    "alice-chen", "bob-martinez", "cara-nguyen", "david-okafor",
    "eve-johnson", "frank-williams", "grace-lee", "henry-brown",
    "iris-garcia", "jack-smith",
]

SYSTEM_NAMES = [
    "apache-iceberg", "flink", "trino", "kafka", "airflow",
    "spark", "postgres", "s3", "glacier", "docker",
    "kubernetes", "prometheus", "grafana", "terraform", "helm",
    "nginx", "redis", "elasticsearch", "vault", "gitlab",
]

VERBS = [
    "migrated", "upgraded", "deployed", "configured", "optimized",
    "monitored", "secured", "scaled", "refactored", "automated",
    "integrated", "deprecated", "standardized", "documented", "tested",
]

NOUNS = [
    "production cluster", "staging environment", "data pipeline",
    "monitoring stack", "deployment workflow", "security policy",
    "performance benchmark", "disaster recovery", "cost analysis",
    "capacity planning", "incident response", "compliance audit",
    "data governance", "schema migration", "api gateway",
]

def sprint_date(start, sprint, day_offset=2):
    return (start + timedelta(days=(sprint - 1) * 14 + day_offset)).isoformat()

def ulid_for(kind, n):
    h = hashlib.sha256(f"{kind}-{n:06d}".encode()).hexdigest().upper()[:26]
    return "01" + h[2:]

def count_notes(vault):
    notes = os.path.join(vault, "notes")
    if not os.path.isdir(notes):
        return 0
    total = 0
    for _, _, files in os.walk(notes):
        total += sum(1 for f in files if f.endswith(".md"))
    return total

def copy_base_vault(base_vault, out):
    copied = 0
    for sub in ("notes", "raw"):
        src = os.path.join(base_vault, sub)
        if os.path.isdir(src):
            shutil.copytree(src, os.path.join(out, sub), dirs_exist_ok=True)
            if sub == "notes":
                copied = count_notes(base_vault)
    return copied

def fact_seq(sprint, idx, facts_per_sprint):
    return (sprint - 1) * facts_per_sprint + idx + 1

def make_fact(sprint, idx, start, persons, systems, facts_per_sprint):
    topic = TOPICS[(sprint + idx) % len(TOPICS)]
    verb = VERBS[(sprint + idx) % len(VERBS)]
    noun = NOUNS[(sprint + idx) % len(NOUNS)]
    person = persons[(sprint + idx) % len(persons)]
    sys1 = systems[(sprint + idx) % len(systems)]
    sys2 = systems[(sprint + idx + 3) % len(systems)]
    seq = fact_seq(sprint, idx, facts_per_sprint)
    tid = 10000 + seq
    fid = ulid_for("fact", seq)
    title = f"{verb} {topic} {noun}"
    body = (f"During sprint {sprint}, the team {verb} the {topic} system for the "
            f"{noun}. The {sys1} and {sys2} clusters were involved. "
            f"The change was {verb} by {person} after performance benchmarking "
            f"showed a {(random.randint(15, 80))}% improvement in "
            f"{random.choice(['latency', 'throughput', 'reliability', 'cost'])}.")
    links_field = ", ".join(f'"[[{w}]]"' for w in [person, sys1, sys2])
    fm = f"""---
type: fact
id: {fid}
title: "{title}"
status: active
valid_from: {sprint_date(start, sprint)}
confidence: high
source: internal
tags: [{topic.replace(" ", ", ")}, fact]
links: [{links_field}]
---
# DL-{tid} (Sprint {sprint}): {title}

{body}
"""
    return fm, fid, [person, sys1, sys2]

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", default="/tmp/kgx-large")
    parser.add_argument("--nodes", type=int, default=10000,
                       help="Target number of note nodes")
    parser.add_argument("--edges", type=int, default=15000,
                       help="Target number of edges")
    parser.add_argument("--facts-per-sprint", type=int, default=20,
                       help="Facts generated per sprint")
    parser.add_argument("--base-vault",
                       help="Existing KGX vault whose notes/raw files are preserved in the output")
    args = parser.parse_args()

    out = args.out
    os.makedirs(out, exist_ok=True)
    for sub in ["notes/entities", "notes/facts", "notes/decisions",
                "notes/experiences", "notes/incidents", "raw"]:
        os.makedirs(os.path.join(out, sub), exist_ok=True)

    start = date(2025, 10, 6)
    base_notes = count_notes(args.base_vault) if args.base_vault else 0

    # Calculate required sprints
    synthetic_target_nodes = max(0, args.nodes - base_notes)
    target_facts = synthetic_target_nodes - len(PERSON_NAMES) - len(SYSTEM_NAMES) - 20
    sprints = max(10, target_facts // args.facts_per_sprint + 1)
    f_per_sprint = args.facts_per_sprint

    print(f"Generating ~{sprints * f_per_sprint + len(PERSON_NAMES) + len(SYSTEM_NAMES) + 20} notes", file=sys.stderr)

    # Entities
    for i, name in enumerate(PERSON_NAMES, 1):
        eid = ulid_for("person", i)
        fm = f"""---
type: entity
id: {eid}
entity_type: person
title: {name.replace('-', ' ').title()}
status: active
valid_from: {sprint_date(start, 1)}
tags: [team, person, engineering]
links: []
---
# {name.replace('-', ' ').title()}

Team member. Active since sprint 1.
"""
        with open(os.path.join(out, f"notes/entities/{name}.md"), "w") as f:
            f.write(fm)

    for i, name in enumerate(SYSTEM_NAMES, 1):
        eid = ulid_for("system", i)
        owner = PERSON_NAMES[i % len(PERSON_NAMES)]
        fm = f"""---
type: entity
id: {eid}
entity_type: system
title: {name}
status: active
valid_from: {sprint_date(start, 1)}
tags: [system, infrastructure]
links: ["[[{owner}]]"]
---
# {name}

Infrastructure component. Owner: {owner.replace('-', ' ').title()}.
"""
        with open(os.path.join(out, f"notes/entities/{name}.md"), "w") as f:
            f.write(fm)

    edge_count = 0
    total_notes = len(PERSON_NAMES) + len(SYSTEM_NAMES)

    # Facts
    for sprint in range(1, sprints + 1):
        for idx in range(f_per_sprint):
            fm, fid, links = make_fact(sprint, idx, start, PERSON_NAMES, SYSTEM_NAMES, f_per_sprint)
            fname = f"dl-{sprint:04d}-{idx:02d}.md"
            with open(os.path.join(out, f"notes/facts/{fname}"), "w") as f:
                f.write(fm)
            edge_count += len(links)
            total_notes += 1

        if sprint % 50 == 0:
            print(f"  sprint {sprint}/{sprints} ({total_notes} notes so far)", file=sys.stderr)

    # ADRs every 4 sprints
    for sprint in range(4, sprints + 1, 4):
        topic = TOPICS[(sprint // 4) % len(TOPICS)]
        person = PERSON_NAMES[(sprint // 4) % len(PERSON_NAMES)]
        aid = ulid_for("adr", sprint // 4)
        fm = f"""---
type: decision
id: {aid}
title: "ADR regarding {topic}"
status: active
valid_from: {sprint_date(start, sprint)}
decided_by: {person}
tags: [adr, architecture]
links: ["[[{person}]]"]
---
# ADR (Sprint {sprint}): {topic}

Decision recorded regarding {topic}. Owner: {person.replace('-', ' ').title()}.
"""
        with open(os.path.join(out, f"notes/decisions/adr-s{sprint}.md"), "w") as f:
            f.write(fm)
        edge_count += 1
        total_notes += 1

    # Experiences every 6 sprints
    for sprint in range(3, sprints + 1, 6):
        topic = TOPICS[(sprint // 6) % len(TOPICS)]
        person = PERSON_NAMES[(sprint // 6) % len(PERSON_NAMES)]
        lid = ulid_for("exp", sprint)
        fm = f"""---
type: experience
id: {lid}
title: "Lesson learned: {topic}"
status: active
valid_from: {sprint_date(start, sprint)}
tags: [lesson, experience]
links: ["[[{person}]]"]
---
# Lesson (Sprint {sprint}): {topic}

Key learning about {topic} documented by {person.replace('-', ' ').title()}.
"""
        with open(os.path.join(out, f"notes/experiences/exp-s{sprint}.md"), "w") as f:
            f.write(fm)
        edge_count += 1
        total_notes += 1

    # Sprint ceremonies
    for sprint in range(1, sprints + 1, 2):
        cid = ulid_for("cer", sprint)
        fm = f"""---
type: experience
id: {cid}
title: Sprint {sprint} planning
status: active
valid_from: {sprint_date(start, sprint)}
tags: [ceremony, planning]
links: []
---
# Sprint {sprint} planning

Sprint planning session.
"""
        with open(os.path.join(out, f"notes/facts/sprint-{sprint:04d}-planning.md"), "w") as f:
            f.write(fm)
        total_notes += 1

    # Add some superseded facts (for temporal questions)
    for i in range(min(10, sprints * f_per_sprint // 20)):
        later_sprint = max(1, (i * 20) % sprints + 1)
        early_sprint = max(1, (i * 20 + 5) % sprints + 1)
        later_idx = i % f_per_sprint
        early_idx = i % f_per_sprint
        later_fid = ulid_for("fact", fact_seq(later_sprint, later_idx, f_per_sprint))
        early_fid = ulid_for("fact", fact_seq(early_sprint, early_idx, f_per_sprint))
        # Overwrite early fact with superseded status
        topic = TOPICS[(early_sprint + early_idx) % len(TOPICS)]
        verb = VERBS[(early_sprint + early_idx) % len(VERBS)]
        noun = NOUNS[(early_sprint + early_idx) % len(NOUNS)]
        person = PERSON_NAMES[(early_sprint + early_idx) % len(PERSON_NAMES)]
        sys1 = SYSTEM_NAMES[(early_sprint + early_idx) % len(SYSTEM_NAMES)]
        tid = 10000 + fact_seq(early_sprint, early_idx, f_per_sprint)
        body = f"Superseded by later finding. Initial {verb} of {topic} for the {noun}."
        links_field = f'"[[{person}]]", "[[{sys1}]]"'
        fm = f"""---
type: fact
id: {early_fid}
title: "(superseded) {verb} {topic} {noun}"
status: superseded
valid_from: {sprint_date(start, early_sprint)}
valid_to: {sprint_date(start, later_sprint)}
superseded_by: {later_fid}
tags: [{topic.replace(" ", ", ")}, fact]
links: [{links_field}]
---
# DL-{tid} (Sprint {early_sprint}): {verb} {topic}

{body}
"""
        fname = f"dl-{early_sprint:04d}-{early_idx:02d}.md"
        with open(os.path.join(out, f"notes/facts/{fname}"), "w") as f:
            f.write(fm)

    copied_base_notes = copy_base_vault(args.base_vault, out) if args.base_vault else 0

    manifest = {
        "nodes": total_notes + copied_base_notes,
        "synthetic_nodes": total_notes,
        "base_nodes": copied_base_notes,
        "edges": edge_count,
        "sprints": sprints,
        "facts_per_sprint": f_per_sprint,
        "persons": len(PERSON_NAMES),
        "systems": len(SYSTEM_NAMES),
        "seed": SEED,
    }
    with open(os.path.join(out, "manifest.json"), "w") as f:
        json.dump(manifest, f, indent=2)

    print(json.dumps(manifest, indent=2))

if __name__ == "__main__":
    main()
