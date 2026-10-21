#!/bin/zsh
# W0 proof of the wasm32-mvp profile (docs/PLANO-WASM-MVP.md): builds the scalar
# module from tests/wasm/w0 (LLVM IR + a no_std Rust fragment), gates it as
# strict MVP, runs the same file in the independent executor (Wasmi), in Node
# (V8) and in headless Chrome, and compares hashes and output with values
# computed independently. Writes the evidence to $OUT (default
# bench/results/2026-10-21-wasm-w0).
#
# Requires: DARTFORGE_CLANG (LLVM 22.1.8 with lld), rustc with wasm32v1-none,
# node, and Google Chrome (CHROME to override the path).
set -euo pipefail
ROOT=${0:A:h:h}
SRC=$ROOT/tests/wasm/w0
OUT=${OUT:-$ROOT/bench/results/2026-10-21-wasm-w0}
WORK=${WORK:-${TMPDIR:-/tmp}/dartforge-wasm-w0}
CLANG=${DARTFORGE_CLANG:?set DARTFORGE_CLANG to LLVM 22.1.8 clang}
LLD=${CLANG:h}/lld
NODE=${NODE:-node}
CHROME=${CHROME:-"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"}
mkdir -p $WORK $OUT

# 1. Build: the generated-code side and the Rust side, both for the MVP CPU.
$CLANG --target=wasm32-unknown-unknown -mcpu=mvp -O2 -c $SRC/scalar.ll -o $WORK/scalar.o
rustc --edition 2024 --target wasm32v1-none -C target-cpu=mvp -C panic=abort -C opt-level=2 \
  --crate-type staticlib --crate-name dfrt $SRC/rt.rs -o $WORK/librt.a
$LLD -flavor wasm $WORK/scalar.o $WORK/librt.a -o $WORK/w0.wasm --no-entry \
  --export=dartforge_run --export=dartforge_abi_version \
  -z stack-size=65536 --initial-memory=262144 --max-memory=16777216 --strip-debug
cp $WORK/w0.wasm $OUT/w0.wasm

# 2. Gate: strict MVP, only dartforge_host_v1 imports.
cargo run -q --release -p dartforge-wasm-gate --target-dir $ROOT/target/wasm -- \
  $OUT/w0.wasm --export dartforge_run --export dartforge_abi_version --json > $OUT/gate.json
SHA=$(python3 -c "import json,sys;print(json.load(open('$OUT/gate.json'))['sha256'])")

# 2b. Negative control: the same IR for LLVM's generic CPU uses post-MVP
# instructions (sign extension), and the gate must refuse it.
$CLANG --target=wasm32-unknown-unknown -mcpu=generic -O2 -c $SRC/scalar.ll -o $WORK/scalar-generic.o
$LLD -flavor wasm $WORK/scalar-generic.o $WORK/librt.a -o $WORK/w0-generic.wasm --no-entry \
  --export=dartforge_run --export=dartforge_abi_version -z stack-size=65536 --strip-debug
if cargo run -q --release -p dartforge-wasm-gate --target-dir $ROOT/target/wasm -- \
  $WORK/w0-generic.wasm --export dartforge_run > /dev/null 2> $OUT/negative-control.txt; then
  echo "W0 FAILED: the gate accepted the generic-CPU module"; exit 1
fi
sed -i.bak "s|$WORK/||" $OUT/negative-control.txt && rm $OUT/negative-control.txt.bak

# 3. The same file in three hosts.
cargo build -q --release -p dartforge-wasm-host --target-dir $ROOT/target/wasm
DARTFORGE_WASM_REPORT=1 $ROOT/target/wasm/release/dartforge-wasm-run $OUT/w0.wasm \
  > $OUT/wasmi.stdout 2> $OUT/wasmi.stderr; echo $? > $OUT/wasmi.status
DARTFORGE_WASM_REPORT=1 $NODE $ROOT/runtime/wasm/run_node.mjs $OUT/w0.wasm \
  > $OUT/node.stdout 2> $OUT/node.stderr; echo $? > $OUT/node.status
$NODE $ROOT/runtime/wasm/browser_page.mjs $OUT/w0.wasm $WORK/w0.html
"$CHROME" --headless=new --disable-gpu --no-first-run --no-default-browser-check \
  --user-data-dir=$WORK/chrome-profile --virtual-time-budget=10000 \
  --dump-dom "file://$WORK/w0.html" > $WORK/chrome.dom 2> $WORK/chrome.log
python3 - "$WORK/chrome.dom" "$OUT/chrome.json" <<'PY'
import html, re, sys
dom = open(sys.argv[1]).read()
m = re.search(r'<pre id="result">(.*?)</pre>', dom, re.S)
open(sys.argv[2], "w").write(html.unescape(m.group(1)) if m else '{"error":"no result"}')
PY

# 4. Expected values, computed independently of LLVM and the hosts.
python3 - "$OUT" "$SHA" <<'PY'
import json, sys
out, sha = sys.argv[1], sys.argv[2]
def wrap(v): return (v + 2**63) % 2**64 - 2**63
def fib(n): return n if n < 2 else fib(n - 1) + fib(n - 2)
def trunc_div(a, b): q = abs(a) // abs(b); return q if (a < 0) == (b < 0) else -q
expected = [
    ("f", fib(30)),
    ("e", ((fib(30) + 160) & 0xFF) - (256 if (fib(30) + 160) & 0x80 else 0)),
    ("m", wrap((2**63 - 1) * 3)),
    ("d", trunc_div(-7, 2)),
    ("r", -7 - trunc_div(-7, 2) * 2),
    ("a", -1024 >> 3),
    ("l", (2**64 - 1) >> 60),
    ("x", int(1.0e10 / 3.0)),
    ("t", 42),
    ("u", -21),
    ("h", ((2**63 - 1) * (2**63 - 1)) >> 64),
    ("s", wrap(1099511627776 - 5 + 2**63 - 1)),
    ("v", sum(i * i for i in range(1000)) * 100 + len(f"1000-{sum(i * i for i in range(1000))}")),
]
text = "".join(f"{k}={v}\n" for k, v in expected)
chrome = json.load(open(f"{out}/chrome.json"))
hosts = {
    "wasmi": {"stdout": open(f"{out}/wasmi.stdout").read(), "status": int(open(f"{out}/wasmi.status").read()),
              "sha256": open(f"{out}/wasmi.stderr").read().split("sha256 ")[-1].strip()},
    "node": {"stdout": open(f"{out}/node.stdout").read(), "status": int(open(f"{out}/node.status").read()),
             "sha256": open(f"{out}/node.stderr").read().split("sha256 ")[-1].strip()},
    "chrome": {"stdout": chrome.get("stdout"), "status": chrome.get("status"), "sha256": chrome.get("sha256"),
               "userAgent": chrome.get("userAgent"), "trap": chrome.get("trap"), "error": chrome.get("error")},
}
ok = all(h["stdout"] == text and h["status"] == 0 and h["sha256"] == sha for h in hosts.values())
summary = {"sha256": sha, "expected_stdout": text, "hosts": hosts, "passed": ok}
json.dump(summary, open(f"{out}/summary.json", "w"), indent=2)
print(json.dumps({k: (v["status"], v["sha256"] == sha, v["stdout"] == text) for k, v in hosts.items()}))
print("W0 passed" if ok else "W0 FAILED")
sys.exit(0 if ok else 1)
PY
