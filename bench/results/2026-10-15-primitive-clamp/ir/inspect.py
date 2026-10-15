"""Inspect existing immutable OFF/ON LLVM; never compile or execute a binary."""
import hashlib
import json
from pathlib import Path
import re

ROOT = Path.cwd()
ARCHIVE = Path(__file__).resolve().parent
PRODUCER = ROOT / "target/primitive-clamp-producer/dartforge"
EXPECTED = "2ae87eaecb64da6398da7fd32fc879e6f7cb7ae6f01ebf977be5c593c81080ce"


def digest(data):
    return hashlib.sha256(data).hexdigest()


assert digest(PRODUCER.read_bytes()) == EXPECTED
validation_path = ARCHIVE.parent / "native/validation.json"
validation = json.loads(validation_path.read_text())
assert validation["inputs_unchanged"]
report = {
    "producer_sha256": EXPECTED,
    "native_validation_sha256": digest(validation_path.read_bytes()),
    "scope": "Existing app LLVM after HIR optimization, before Clang optimization. "
    "Selected bodies only; this establishes exercised guarded clamp paths and "
    "retained fallback/check code, not runtime speed or universal equivalence.",
    "functions": {},
    "files": {},
}
selected = {
    "clamp": ["unitClamp", "typedUnitClamp", "genericClamp"],
    "color": ["convertFormatValue"],
}
for case, names in selected.items():
    report["functions"][case] = {}
    for profile in ["off", "on"]:
        profile_validation = validation["cases"][case]["profiles"][profile]
        assert profile_validation["normal"]["exact"]
        assert profile_validation["stress"]["exact"]
        path = ROOT / "target/primitive-clamp-native" / case / profile / ".df_tmp/program.ll"
        data = path.read_bytes()
        expected = profile_validation["ir"][".df_tmp/program.ll"]["sha256"]
        assert digest(data) == expected
        report["files"][str(path.relative_to(ROOT))] = {
            "sha256": expected, "bytes": len(data),
        }
        functions = re.findall(r"^define [^\n]*\{\n.*?^\}\n", data.decode(), re.M | re.S)
        for name in names:
            matches = [body for body in functions if f".{name}(" in body.splitlines()[0]]
            assert len(matches) == 1, (case, profile, name)
            body = matches[0]
            destination = ARCHIVE / case / profile / (name + ".ll")
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text(body)
            calls = re.findall(r"call i64 @([^\s(]+\.clamp)\(", body)
            row = {
                "symbol": re.search(r"@([^\s(]+)\(", body).group(1),
                "body_sha256": digest(body.encode()), "bytes": len(body.encode()),
                "nan_bit_guards": len(re.findall(r"icmp sgt i64 [^\n]*, 9218868437227405312", body)),
                "negative_zero_bit_guards": len(re.findall(r"icmp eq i64 [^\n]*, -9223372036854775808", body)),
                "double_to_bits": len(re.findall(r"bitcast double [^\n]* to i64", body)),
                "pending_loads": len(re.findall(r"load i8, ptr %ctx,", body)),
                "integer_boxes": len(re.findall(r"call i64 @df\.box_int\(", body)),
                "double_boxes": len(re.findall(r"call i64 @df\.box_double\(", body)),
                "canonical_static_fallbacks": calls,
                "selector_fallbacks": len(re.findall(r"call ptr @df\.selector\(", body)),
                "extracted": str(destination.relative_to(ROOT)),
            }
            report["functions"][case].setdefault(name, {})[profile] = row
    for name in names:
        off = report["functions"][case][name]["off"]
        on = report["functions"][case][name]["on"]
        assert off["nan_bit_guards"] == 0
        if name == "genericClamp":
            assert on["nan_bit_guards"] == 0
            assert off["body_sha256"] == on["body_sha256"]
        else:
            assert on["nan_bit_guards"] > 0
            assert on["negative_zero_bit_guards"] == on["nan_bit_guards"]
        for metric in ["pending_loads", "integer_boxes", "double_boxes", "selector_fallbacks"]:
            assert off[metric] == on[metric], (name, metric, off[metric], on[metric])
        assert off["canonical_static_fallbacks"] == on["canonical_static_fallbacks"]

report["identity_evidence"] = {
    "unitClamp_on": "The emitted i64 result Phi selects the original receiver or "
    "original boxed int bounds on fast paths, and the original selector result on fallback. "
    "The original context pending load/check remains after this join.",
    "semantics_oracle": "373 clamp fixture lines and 2736 color lines match the official "
    "VM and official AOT in both OFF/ON and normal/GC-stress runs; see bound validation.",
}
(ARCHIVE / "report.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps({case: {name: rows["on"]["nan_bit_guards"] for name, rows in functions.items()}
                  for case, functions in report["functions"].items()}))
