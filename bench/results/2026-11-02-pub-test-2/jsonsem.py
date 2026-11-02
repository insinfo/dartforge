"""Order-independent view of a JSON reporter stream.

usage: jsonsem.py <file>

With concurrent suites the interleaving of events and therefore the ids
depend on timing. This prints, sorted, one line per test with its suite path,
group names, metadata, prints, errors (without frames) and final result, and
one line per group, plus the final `done` and `allSuites` events, so two runs
can be compared without the timing-dependent order.
"""
import json
import sys

suites, groups, tests = {}, {}, {}
lines = []
for raw in open(sys.argv[1], encoding="utf-8"):
    try:
        e = json.loads(raw)
    except ValueError:
        lines.append("TEXT " + raw.rstrip("\n"))
        continue
    t = e.get("type")
    if t == "suite":
        suites[e["suite"]["id"]] = e["suite"]["path"]
    elif t == "group":
        g = e["group"]
        groups[g["id"]] = g
        lines.append("group %s | %r | %s | count=%d" % (suites[g["suiteID"]], g["name"], json.dumps(g["metadata"], sort_keys=True), g["testCount"]))
    elif t == "testStart":
        x = e["test"]
        tests[x["id"]] = {"suite": suites[x["suiteID"]], "name": x["name"], "groups": [groups[i]["name"] for i in x["groupIDs"]], "metadata": x["metadata"], "prints": [], "errors": []}
    elif t == "print":
        tests[e["testID"]]["prints"].append((e["messageType"], e["message"]))
    elif t == "error":
        tests[e["testID"]]["errors"].append((e["error"], e["isFailure"]))
    elif t == "testDone":
        x = tests[e["testID"]]
        x["result"] = (e["result"], e["skipped"], e["hidden"])
    elif t == "done":
        lines.append("done success=%s" % e["success"])
    elif t == "allSuites":
        lines.append("allSuites count=%d" % e["count"])
    elif t == "start":
        lines.append("start %s %s" % (e["protocolVersion"], e["runnerVersion"]))
for x in tests.values():
    lines.append("test %s | %r | %r | %s | prints=%r | errors=%r | result=%r" % (x["suite"], x["name"], x["groups"], json.dumps(x["metadata"], sort_keys=True), x["prints"], x["errors"], x.get("result")))
for line in sorted(lines):
    print(line)
