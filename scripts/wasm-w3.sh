#!/bin/zsh
# W3 proof (docs/PLANO-WASM-MVP.md): Dart programs compiled by
# `dartforge compile-wasm` — tests/wasm/w3 and programs of corpus/native — run
# with the same SHA-256 and the output of `dart run` in Wasmtime
# (dartforge-wasm-run), Node and headless Chrome. The timers program waits in
# the browser through the host's `setTimeout` (dartforge_resume). Evidence in
# $OUT (default bench/results/2026-10-21-wasm-w3).
#
# Requires: a dartforge built with --features native (DARTFORGE_BIN), DART (the
# 3.6.2 SDK's dart), DARTFORGE_CLANG, node and Google Chrome (CHROME).
set -euo pipefail
ROOT=${0:A:h:h}
OUT=${OUT:-$ROOT/bench/results/2026-10-21-wasm-w3}
WORK=${WORK:-${TMPDIR:-/tmp}/dartforge-wasm-w3}
DARTFORGE=${DARTFORGE_BIN:-$ROOT/target/release/dartforge}
RUN=${DARTFORGE_WASM_RUN:-${DARTFORGE:h}/dartforge-wasm-run}
DART=${DART:-dart}
NODE=${NODE:-node}
CHROME=${CHROME:-"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"}
: ${DARTFORGE_CLANG:?set DARTFORGE_CLANG to LLVM 22.1.8 clang}
mkdir -p $WORK $OUT
PROGRAMS=(
  $ROOT/tests/wasm/w3/hello.dart
  $ROOT/tests/wasm/w3/timers.dart
  $ROOT/corpus/native/26_for_in_de_lista.dart
  $ROOT/corpus/native/28_closures_tipadas.dart
  $ROOT/corpus/native/101_json_pilha_e_mapas.dart
)
for p in $PROGRAMS; do
  t=${p:t:r}
  $DART run $p > $OUT/$t.vm.stdout
  $DARTFORGE compile-wasm $p -o $OUT/$t.wasm
  cargo run -q --release -p dartforge-wasm-gate --target-dir $ROOT/target/wasm -- \
    $OUT/$t.wasm --export dartforge_run --export dartforge_abi_version --json > $OUT/$t.gate.json
  DARTFORGE_WASM_REPORT=1 $RUN $OUT/$t.wasm > $OUT/$t.wasmtime.stdout 2> $OUT/$t.wasmtime.stderr
  DARTFORGE_WASM_REPORT=1 $NODE $ROOT/runtime/wasm/run_node.mjs $OUT/$t.wasm \
    > $OUT/$t.node.stdout 2> $OUT/$t.node.stderr
  $NODE $ROOT/runtime/wasm/browser_page.mjs $OUT/$t.wasm $WORK/$t.html
  "$CHROME" --headless=new --disable-gpu --no-first-run --no-default-browser-check \
    --user-data-dir=$WORK/chrome-profile --virtual-time-budget=60000 \
    --dump-dom "file://$WORK/$t.html" > $WORK/$t.chrome.dom 2> $WORK/$t.chrome.log
  python3 - "$WORK/$t.chrome.dom" "$OUT/$t.chrome.json" <<'PY'
import html, re, sys
dom = open(sys.argv[1]).read()
m = re.search(r'<pre id="result">(.*?)</pre>', dom, re.S)
open(sys.argv[2], "w").write(html.unescape(m.group(1)) if m else '{"error":"no result"}')
PY
done
python3 - "$OUT" ${PROGRAMS:t:r} <<'PY'
import json, sys
out, names = sys.argv[1], sys.argv[2:]
summary, ok = {}, True
for t in names:
    vm = open(f"{out}/{t}.vm.stdout").read()
    gate = json.load(open(f"{out}/{t}.gate.json"))
    chrome = json.load(open(f"{out}/{t}.chrome.json"))
    hosts = {
        "wasmtime": (open(f"{out}/{t}.wasmtime.stdout").read(), open(f"{out}/{t}.wasmtime.stderr").read().split("sha256 ")[-1].strip()),
        "node": (open(f"{out}/{t}.node.stdout").read(), open(f"{out}/{t}.node.stderr").read().split("sha256 ")[-1].strip()),
        "chrome": (chrome.get("stdout"), chrome.get("sha256")),
    }
    result = {h: {"same_output": o == vm, "same_hash": s == gate["sha256"]} for h, (o, s) in hosts.items()}
    ok = ok and all(r["same_output"] and r["same_hash"] for r in result.values())
    summary[t] = {"sha256": gate["sha256"], "bytes": gate["bytes"], "imports": gate["imports"], "hosts": result,
                  "chrome": chrome.get("userAgent")}
summary["passed"] = ok
json.dump(summary, open(f"{out}/summary.json", "w"), indent=2)
for t in names:
    print(t, summary[t]["bytes"], {h: r["same_output"] and r["same_hash"] for h, r in summary[t]["hosts"].items()})
print("W3 passed" if ok else "W3 FAILED")
sys.exit(0 if ok else 1)
PY
