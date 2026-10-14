"""Count static call sites in one retained function before final LLVM LTO."""
import collections
import json
import re
import sys
from pathlib import Path

path, wanted = Path(sys.argv[1]), sys.argv[2]
body = []
with path.open() as stream:
    for line in stream:
        if not body and line.startswith("define ") and wanted in line and "$tear" not in line:
            body.append(line)
            continue
        if body:
            body.append(line)
            if line.strip() == "}":
                break
assert body, wanted
text = "".join(body)
direct = collections.Counter(re.findall(r"\bcall\s+[^@%\n]*@([^\s(]+)\(", text))
indirect = len(re.findall(r"\bcall\s+[^@%\n]*%[^\s(]+\(", text))
print(json.dumps({
    "file": str(path.resolve()), "function_header": body[0].strip(),
    "ir_lines": len(body), "direct_call_sites": sum(direct.values()),
    "indirect_call_sites": indirect, "direct_call_site_counts": dict(direct.most_common()),
    "limitations": "Static LLVM IR sites before final LTO; not execution counts or final machine-code instruction counts."
}, indent=2))
