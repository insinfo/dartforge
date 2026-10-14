"""Attribute linked text intervals without counting symbol aliases twice."""
import collections
import hashlib
import json
import re
import sys
from pathlib import Path


def category(name):
    if name.startswith("_df.package"):
        match = re.match(r"_df\.package\$3a([^$]+)", name)
        return "package:" + (match.group(1) if match else "unknown")
    if name.startswith("_df.dart"):
        match = re.match(r"_df\.dart\$3a([^.$]+)", name)
        return "sdk:" + (match.group(1) if match else "unknown")
    if name.startswith("_df."):
        return "generated-support"
    return "runtime-and-native-support"


def library(name):
    decoded = re.sub(r"\$([0-9a-f]{2})", lambda match: chr(int(match.group(1), 16)), name)
    match = re.match(r"_df\.(package:.*?\.dart|dart:[^.]+)", decoded)
    return match.group(1) if match else category(name)


def main():
    path = Path(sys.argv[1])
    source = path.read_text()
    section = re.search(r"^(0x[\dA-Fa-f]+)\s+(0x[\dA-Fa-f]+)\s+__TEXT\s+__text$", source, re.M)
    start, size = (int(value, 16) for value in section.groups())
    end = start + size
    symbols = source.split("# Symbols:\n", 1)[1].split("# Dead Stripped Symbols:", 1)[0]
    events = collections.defaultdict(list)
    rows = []
    for match in re.finditer(r"^(0x[\dA-Fa-f]+)\s+(0x[\dA-Fa-f]+)\s+\[\s*(\d+)\]\s+(.+)$", symbols, re.M):
        address, length = (int(value, 16) for value in match.groups()[:2])
        lo, hi = max(start, address), min(end, address + length)
        if lo >= hi:
            continue
        label = category(match.group(4))
        index = len(rows)
        rows.append({"address": address, "bytes": length, "name": match.group(4), "category": label, "library": library(match.group(4))})
        events[lo].append((index, True))
        events[hi].append((index, False))
    events[start]
    events[end]
    active = set()
    totals = collections.Counter()
    libraries = collections.Counter()
    previous = start
    for position in sorted(events):
        labels = {rows[index]["category"] for index in active}
        label = next(iter(labels)) if len(labels) == 1 else ("unattributed-padding" if not labels else "mixed-alias-interval")
        totals[label] += position - previous
        owners = {rows[index]["library"] for index in active}
        owner = next(iter(owners)) if len(owners) == 1 else ("unattributed-padding" if not owners else "mixed-alias-interval")
        libraries[owner] += position - previous
        for index, added in events[position]:
            if added:
                active.add(index)
            else:
                active.remove(index)
        previous = position
    assert sum(totals.values()) == size
    assert sum(libraries.values()) == size
    print(json.dumps({
        "map": str(path.resolve()), "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "text_bytes": size, "category_bytes": dict(totals.most_common()),
        "largest_libraries": dict(libraries.most_common(40)),
        "largest_symbols": sorted(rows, key=lambda row: row["bytes"], reverse=True)[:40],
        "limitations": "Symbol provenance attributes linked bytes, not reachability or execution cost. Cross-category aliases remain mixed; upstream snapshot layout needs separate REA analysis."
    }, indent=2))


if __name__ == "__main__":
    main()
