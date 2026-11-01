"""Runs the editing session of `examples/editing_latency.rs` against
DartForge's server and the official one (`dart language-server`), rotating
them, and prints the medians side by side and the agreement of the final
diagnostics.

    python3 crates/lsp/oracle/editing_compare.py --driver <editing_latency binary> \
        --server dartforge=target/release/dartforge-lsp \
        --server before=<an older dartforge-lsp> \
        --dart <sdk>/bin/dart \
        --rounds 3 --out <dir> \
        --project name=<n>,root=<root>,file=<lib/a.dart>,private=<lib/a.dart>,api=<lib/b.dart> \
        [-- <options for the driver>]

Every project runs `--rounds` times per server, the order of the servers
rotating between rounds (the machine is shared: a slow minute hits all of
them). A latency is the median of all the samples of all rounds; startup,
burst and memory are the median of the rounds. `--report-only` reads the
JSON files of `--out` again without running anything.

Diagnostics agreement: the last publication of every file at the end of a
session (every edit undone), keyed by (file, start line, code); the share of
the official server's diagnostics that each DartForge server also publishes,
and the ones that only one of the two publishes, by code.
"""

import argparse
import collections
import json
import os
import statistics
import subprocess
import sys
import time
import urllib.parse

ROWS = [
    ("startup", "startup to the settled first analysis"),
    ("startup: cpu", "startup: CPU time of all processes"),
    ("keystroke: settled", "keystroke in a body → settled"),
    ("keystroke: cpu", "keystroke: CPU time"),
    ("burst: settled", "burst of keystrokes → settled"),
    ("body edit: diagnostic published", "body edit → diagnostic published"),
    ("body edit: settled", "body edit → settled"),
    ("body edit: cpu", "body edit: CPU time"),
    ("private API edit: diagnostic published", "private API edit → diagnostic published"),
    ("private API edit: settled", "private API edit → settled"),
    ("private API edit: cpu", "private API edit: CPU time"),
    ("exported API edit: diagnostic published", "exported (barrel) API edit → diagnostic published"),
    ("exported API edit: settled", "exported (barrel) API edit → settled"),
    ("exported API edit: cpu", "exported API edit: CPU time"),
]
QUERIES = ["hover", "completion", "definition", "references", "prepareRename", "codeAction"]
for q in QUERIES:
    ROWS.append((f"{q}: after edit", f"{q}, first after an edit"))
    ROWS.append((f"{q}: after edit cpu", f"{q}, first after an edit: server CPU"))
    ROWS.append((f"{q}: warm", f"{q}, no edit in between"))
    ROWS.append((f"{q}: warm cpu", f"{q}, no edit in between: server CPU"))
MEMORY = [
    ("total_peak_mib", "all processes: footprint peak (MiB)"),
    ("total_mean_mib", "all processes: footprint mean (MiB)"),
    ("total_rest_mib", "all processes: footprint at rest (MiB)"),
    ("rss_peak_mib", "all processes: RSS peak (MiB)"),
]
DART = "dart"


def project_arg(text):
    fields = dict(kv.split("=", 1) for kv in text.split(","))
    for key in ("name", "root", "file"):
        if key not in fields:
            raise argparse.ArgumentTypeError(f"--project needs {key}=")
    return fields


def server_arg(text):
    label, _, path = text.partition("=")
    if not path or label == DART:
        raise argparse.ArgumentTypeError("--server needs <label>=<path>, label not 'dart'")
    return label, path


def run(args, server, project, round_):
    out = os.path.join(args.out, f"{project['name']}.{server}.{round_}.json")
    cmd = [args.driver]
    if server == DART:
        cmd += ["--dart", args.dart, "--label", DART]
    else:
        cmd += ["--server", dict(args.server)[server], "--label", server]
    cmd += ["--root", project["root"], "--file", project["file"], "--json", out]
    for key in ("private", "api"):
        if key in project:
            cmd += [f"--{key}", project[key]]
    cmd += args.extra
    load = os.getloadavg()[0]
    uptime = subprocess.run(["uptime"], capture_output=True, text=True).stdout.strip()
    started = time.time()
    log = os.path.join(args.out, f"{project['name']}.{server}.{round_}.log")
    with open(log, "w") as f:
        f.write(f"load {load:.1f}; {uptime}\n")
        f.flush()
        code = subprocess.call(cmd, stdout=f, stderr=subprocess.STDOUT)
    print(
        f"{project['name']} {server} round {round_}: exit {code}, {time.time() - started:.0f} s, load {load:.1f}",
        file=sys.stderr,
        flush=True,
    )
    if code == 0:
        with open(out) as f:
            data = json.load(f)
        data["load"] = load
        data["load_after"] = os.getloadavg()[0]
        with open(out, "w") as f:
            json.dump(data, f)


def load_runs(args, name, server):
    runs = []
    for r in range(args.rounds):
        path = os.path.join(args.out, f"{name}.{server}.{r}.json")
        if os.path.exists(path):
            with open(path) as f:
                runs.append(json.load(f))
    return runs


def median_of(runs, key):
    pooled = []
    for r in runs:
        v = r["latency_ms"].get(key)
        if v is None:
            continue
        pooled.extend(v if isinstance(v, list) else [v])
    return statistics.median(pooled) if pooled else None


def fmt(v):
    if v is None:
        return "—"
    if v < 10:
        return f"{v:.1f}".replace(".", ",")
    return f"{v:,.0f}".replace(",", ".")


def diagnostics_keys(run, root):
    keys = collections.Counter()
    for uri, diags in run.get("diagnostics", {}).items():
        path = urllib.parse.unquote(urllib.parse.urlparse(uri).path)
        rel = os.path.relpath(path, root)
        for d in diags or []:
            code = d.get("code")
            if isinstance(code, dict):
                code = code.get("value")
            keys[(rel, d["range"]["start"]["line"], str(code).lower())] += 1
    return keys


def report(args):
    servers = [label for label, _ in args.server] + [DART]
    for project in args.project:
        name = project["name"]
        runs = {s: load_runs(args, name, s) for s in servers}
        loads = [r.get(k, 0) for rs in runs.values() for r in rs for k in ("load", "load_after") if k in r]
        counts = " × ".join(str(len(runs[s])) for s in servers)
        print(f"\n### {name} ({counts} sessions, load {min(loads, default=0):.0f}–{max(loads, default=0):.0f})\n")
        print("| ms | " + " | ".join(servers) + " | " + " | ".join(f"{s}/dart" for s in servers[:-1]) + " |")
        print("|---|" + "---:|" * (2 * len(servers) - 1))
        for key, label in ROWS:
            values = [median_of(runs[s], key) for s in servers]
            if all(v is None for v in values):
                continue
            dart = values[-1]
            ratios = [
                f"{v / dart:.2f}×".replace(".", ",") if v is not None and dart else "—" for v in values[:-1]
            ]
            print(f"| {label} | " + " | ".join(fmt(v) for v in values) + " | " + " | ".join(ratios) + " |")
        for key, label in MEMORY:
            values = [
                statistics.median([r["memory"][key] for r in runs[s]]) if runs[s] else None for s in servers
            ]
            dart = values[-1]
            ratios = [
                f"{v / dart:.2f}×".replace(".", ",") if v is not None and dart else "—" for v in values[:-1]
            ]
            print(f"| {label} | " + " | ".join(fmt(v) for v in values) + " | " + " | ".join(ratios) + " |")
        for s in servers:
            missing = {
                k: sum(r["latency_ms"].get(f"{k}: missing", 0) for r in runs[s])
                for k in ("body edit", "private API edit", "exported API edit")
            }
            if any(missing.values()):
                print(f"\n{s}: edits without the expected diagnostic: {missing}")
        if not runs[DART]:
            continue
        b = diagnostics_keys(runs[DART][-1], project["root"])
        total_b = sum(b.values())
        for s in servers[:-1]:
            if not runs[s]:
                continue
            a = diagnostics_keys(runs[s][-1], project["root"])
            common = sum((a & b).values())
            only_a = a - b
            only_b = b - a
            share = f"{100 * common / total_b:.1f}%" if total_b else "—"
            print(
                f"\nfinal diagnostics, {s} × dart: dart {total_b}, {s} {sum(a.values())}, common {common} "
                f"({share} of dart's); only {s} {sum(only_a.values())}, only dart {sum(only_b.values())}"
            )
            for label, only in ((f"only {s}", only_a), ("only dart", only_b)):
                by_code = collections.Counter()
                for (rel, line, code), n in only.items():
                    by_code[code] += n
                if by_code:
                    print(f"  {label} by code: {dict(by_code.most_common(12))}")
                    for (rel, line, code), _ in list(only.items())[: args.examples]:
                        print(f"    {rel}:{line + 1} {code}")


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--driver", required=True)
    p.add_argument("--server", type=server_arg, action="append", required=True)
    p.add_argument("--dart", required=True)
    p.add_argument("--rounds", type=int, default=3)
    p.add_argument("--out", required=True)
    p.add_argument("--project", type=project_arg, action="append", required=True)
    p.add_argument("--only", help="run only these server labels (comma-separated)")
    p.add_argument("--report-only", action="store_true")
    p.add_argument("--examples", type=int, default=0)
    p.add_argument("extra", nargs="*", help="options passed to the driver (after --)")
    args = p.parse_args()
    os.makedirs(args.out, exist_ok=True)
    servers = [label for label, _ in args.server] + [DART]
    if not args.report_only:
        for r in range(args.rounds):
            for project in args.project:
                k = r % len(servers)
                for server in servers[k:] + servers[:k]:
                    if args.only and server not in args.only.split(","):
                        continue
                    run(args, server, project, r)
    report(args)


if __name__ == "__main__":
    main()
