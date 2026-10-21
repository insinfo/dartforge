#!/bin/zsh
# W2 proof (docs/PLANO-WASM-MVP.md): the real DartForge runtime compiled for
# wasm32-linear (the same variant crates/emit_native/build.rs builds) and driven
# by tests/wasm/w2/runtime.ll — allocation, young and full collections that move
# live strings, 64-bit integers, the host clocks, generator and time zone —
# runs with the same hash and output in Wasmtime, Node and headless Chrome; two
# instances in one host are independent; a module whose memory cannot grow ends
# with "Out of memory" and status 255 (tests/wasm/w2/oom.ll). Evidence in $OUT
# (default bench/results/2026-10-21-wasm-w2).
#
# Requires: DARTFORGE_CLANG (LLVM 22.1.8 with lld), the wasm32-unknown-unknown
# Rust target (rust-toolchain.toml), node, and Google Chrome (CHROME).
set -euo pipefail
ROOT=${0:A:h:h}
SRC=$ROOT/tests/wasm/w2
OUT=${OUT:-$ROOT/bench/results/2026-10-21-wasm-w2}
WORK=${WORK:-${TMPDIR:-/tmp}/dartforge-wasm-w2}
CLANG=${DARTFORGE_CLANG:?set DARTFORGE_CLANG to LLVM 22.1.8 clang}
LLD=${CLANG:h}/lld
NODE=${NODE:-node}
CHROME=${CHROME:-"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"}
mkdir -p $WORK $OUT

# 1. The runtime: crates/emit_native/build.rs's wasm32 variant, same flags.
CARGO_ENCODED_RUSTFLAGS="-Ctarget-feature=+simd128,+tail-call,+extended-const" \
  cargo build -q --release --locked -p dartforge-runtime-static --features aot \
  --target wasm32-unknown-unknown --target-dir $ROOT/target/wasm --config 'profile.release.panic="abort"'
RT=$ROOT/target/wasm/wasm32-unknown-unknown/release/libdartforge_runtime_static.a

# 2. The two programs, linked as crates/emit_native/src/linker_wasm.rs links.
link() { # object, output, max memory
  $LLD -flavor wasm $1 $RT -o $2 --no-entry --export=dartforge_abi_version --export=dartforge_run \
    -z stack-size=8388608 --max-memory=$3 --strip-all
}
for t in runtime oom; do
  $CLANG --target=wasm32-unknown-unknown -mcpu=generic -msimd128 -mtail-call -mextended-const \
    -O2 -c $SRC/$t.ll -o $WORK/$t.o
done
link $WORK/runtime.o $OUT/runtime.wasm 4294967296
link $WORK/oom.o $OUT/oom.wasm 67108864

# 3. Gate: the profile, only dartforge_host_v1 imports.
for t in runtime oom; do
  cargo run -q --release -p dartforge-wasm-gate --target-dir $ROOT/target/wasm -- \
    $OUT/$t.wasm --export dartforge_run --export dartforge_abi_version --json > $OUT/$t.gate.json
done

# 4. Each module in three hosts.
cargo build -q --release -p dartforge-wasm-host --target-dir $ROOT/target/wasm
for t in runtime oom; do
  set +e
  DARTFORGE_WASM_REPORT=1 $ROOT/target/wasm/release/dartforge-wasm-run $OUT/$t.wasm \
    > $OUT/$t.wasmtime.stdout 2> $OUT/$t.wasmtime.stderr; echo $? > $OUT/$t.wasmtime.status
  DARTFORGE_WASM_REPORT=1 $NODE $ROOT/runtime/wasm/run_node.mjs $OUT/$t.wasm \
    > $OUT/$t.node.stdout 2> $OUT/$t.node.stderr; echo $? > $OUT/$t.node.status
  set -e
  $NODE $ROOT/runtime/wasm/browser_page.mjs $OUT/$t.wasm $WORK/$t.html
  "$CHROME" --headless=new --disable-gpu --no-first-run --no-default-browser-check \
    --user-data-dir=$WORK/chrome-profile --virtual-time-budget=30000 \
    --dump-dom "file://$WORK/$t.html" > $WORK/$t.chrome.dom 2> $WORK/$t.chrome.log
  python3 - "$WORK/$t.chrome.dom" "$OUT/$t.chrome.json" <<'PY'
import html, re, sys
dom = open(sys.argv[1]).read()
m = re.search(r'<pre id="result">(.*?)</pre>', dom, re.S)
open(sys.argv[2], "w").write(html.unescape(m.group(1)) if m else '{"error":"no result"}')
PY
done
set +e
$NODE $SRC/two_instances.mjs $OUT/runtime.wasm > $OUT/two_instances.stdout; echo $? > $OUT/two_instances.status
set -e

# 5. The expected results.
python3 - "$OUT" <<'PY'
import json, sys
out = sys.argv[1]
expected = "hello wasm\n-9223372036854775807\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\n"
oom_line = "Out of memory: the WebAssembly memory reached "
def host(t, h):
    if h == "chrome":
        c = json.load(open(f"{out}/{t}.chrome.json"))
        return {"stdout": c.get("stdout"), "stderr": c.get("stderr"), "status": c.get("status"),
                "sha256": c.get("sha256"), "trap": c.get("trap"), "userAgent": c.get("userAgent")}
    err = open(f"{out}/{t}.{h}.stderr").read()
    return {"stdout": open(f"{out}/{t}.{h}.stdout").read(), "stderr": err,
            "status": int(open(f"{out}/{t}.{h}.status").read()), "sha256": err.split("sha256 ")[-1].strip()}
summary, ok = {}, True
for t in ["runtime", "oom"]:
    sha = json.load(open(f"{out}/{t}.gate.json"))["sha256"]
    hosts = {h: host(t, h) for h in ["wasmtime", "node", "chrome"]}
    for h, r in hosts.items():
        same = r["sha256"] == sha
        good = (r["stdout"] == expected and r["status"] == 0) if t == "runtime" else \
               (r["status"] == 255 and r["stderr"].startswith(oom_line) and r["stdout"] == "")
        r["passed"] = same and good
        ok = ok and r["passed"]
    summary[t] = {"sha256": sha, "hosts": hosts}
two = int(open(f"{out}/two_instances.status").read()) == 0 and open(f"{out}/two_instances.stdout").read() == expected
summary["two_instances"] = two
ok = ok and two
summary["passed"] = ok
json.dump(summary, open(f"{out}/summary.json", "w"), indent=2)
print(json.dumps({t: {h: r["passed"] for h, r in summary[t]["hosts"].items()} for t in ["runtime", "oom"]}), "two instances:", two)
print("W2 passed" if ok else "W2 FAILED")
sys.exit(0 if ok else 1)
PY
