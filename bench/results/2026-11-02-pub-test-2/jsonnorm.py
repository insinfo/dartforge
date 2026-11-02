"""Normalizes a package:test JSON reporter stream for comparison.

usage: jsonnorm.py <file> [--keep-frames]

Only what changes between two runs of the same tool is removed: `time`, the
`pid` of `start`, and the stack-frame text of `error.stackTrace`. Unless
--keep-frames is given, the declaration frame fields (`line`, `column`, `url`
and the `root_*` fields) are also replaced by a marker, because DartForge's
native executables have no stack frames (see docs/PUB-E-TEST.md); their
values are listed separately by `frames.py`.
"""
import json
import sys

keep = "--keep-frames" in sys.argv
for raw in open(sys.argv[1], encoding="utf-8"):
    raw = raw.rstrip("\n")
    try:
        event = json.loads(raw)
    except ValueError:
        print("TEXT " + raw)
        continue
    if not isinstance(event, dict):
        print("TEXT " + raw)
        continue
    event.pop("time", None)
    if event.get("type") == "start":
        event["pid"] = "<pid>"
    if event.get("type") == "error":
        event["stackTrace"] = "<frames>"
    if not keep:
        for key in ("test", "group"):
            entry = event.get(key)
            if isinstance(entry, dict):
                for field in ("line", "column", "url", "root_line", "root_column", "root_url"):
                    if field in entry:
                        entry[field] = "<frame>"
    print(json.dumps(event, sort_keys=True))
