"""Follow recorded first-cause retention chains, not all possible callers."""
import csv
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
with path.open() as stream:
    rows = list(csv.DictReader(stream, delimiter="\t"))
index = {row["symbol"]: row for row in rows}
chains = {}
for term in ["convertFormatValue", "Win32Window.handleMessage", "drawPixel", "Object.hash", "PktDecoder.readPktHead"]:
    matches = [row["symbol"] for row in rows if term in row["symbol"] and "$tear" not in row["symbol"]]
    chains[term] = []
    for symbol in matches[:3]:
        name, seen, chain = symbol, set(), []
        while name in index and name not in seen and len(chain) < 50:
            seen.add(name)
            row = index[name]
            chain.append(row)
            name = row["caller"] if row["cause"] == "table" and row["caller"] not in ["root", "-"] else row["parent"]
        chains[term].append({"symbol": symbol, "chain": chain, "stopped_at": name})
print(json.dumps({"report": str(path.resolve()),
                  "limitations": "Compiler records one retention cause per symbol. This does not prove absence of additional legitimate callers.",
                  "chains": chains}, indent=2))
