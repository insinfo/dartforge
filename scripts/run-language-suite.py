#!/usr/bin/env python3
"""Runs the Dart SDK's tests/language suite on the VM, DDC, dart2wasm or DartForge
(JS or WebAssembly).

The suite is the one of a Dart SDK source checkout at a tag (3.6.2 by
default), extracted into a work directory with `setup`; the programs are the
`*_test.dart` files without static-error expectations (no `//#` multitest
markers, `// [analyzer]`, `// [cfe]` or `// [web]` annotations), outside
`vm/`, that import no `dart:io`, `dart:isolate`, `dart:mirrors`, `dart:ffi`,
`dart:html`, `dart:js` or private `dart:_` library.

usage:
  run-language-suite.py setup --checkout DIR [--tag 3.6.2]
  run-language-suite.py vm|ddc|dart2wasm|forge|wasm [--jobs N] [--out NAME] [filter...]
  run-language-suite.py compare [--wasm] OLD NEW   (regressions and fixes, by result name;
                                    --wasm also accepts dart2wasm's results)

A tool's results go to <work>/results/<name>.json as {test: {code, out, err}};
the other tools compare against vm.json and write <name>.fail.txt. With
filters only the matching programs run and the results merge into the file.

`wasm` compiles with `dartforge compile-wasm` (production, as `dart compile
wasm`) and runs the module in `dartforge-wasm-run` (Node through
runtime/wasm/run_node.mjs when it has a `dart:js_interop` companion). Like the
differential's `--wasm` (crates/differential/src/report.rs), a program also
passes when it matches dart2wasm.json where the web platform makes dart2wasm
differ from the VM: equal to dart2wasm (cast messages read as the VM's), or
the VM's output up to an uncaught error with dart2wasm's message; a program the
VM refuses to compile (254) that DartForge refuses too counts apart, as
rejected by both. `dart2wasm`
runs `dart compile wasm` through runtime/wasm/run_dart2wasm.mjs.

Environment: DARTFORGE_LANG_WORK (work directory, default
target/language-suite), DART_SDK (default: `dart` on PATH), NODE (default:
`node` on PATH), FORGE (default target/release/dartforge), WASM_RUN (default
dartforge-wasm-run next to FORGE).
"""
import json, os, subprocess, sys, hashlib, shutil, tarfile, io, concurrent.futures as cf

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
HERE = os.environ.get("DARTFORGE_LANG_WORK", os.path.join(ROOT, "target", "language-suite"))
SDK = os.path.join(HERE, "sdk")


def _dart_sdk():
    if os.environ.get("DART_SDK"):
        return os.environ["DART_SDK"]
    dart = shutil.which("dart")
    return os.path.dirname(os.path.dirname(os.path.realpath(dart))) if dart else ""


DART_SDK = _dart_sdk()
DART = os.path.join(DART_SDK, "bin", "dart")
NODE = os.environ.get("NODE") or shutil.which("node") or "node"
FORGE = os.environ.get("FORGE", os.path.join(ROOT, "target", "release", "dartforge"))
WASM_RUN = os.environ.get("WASM_RUN", os.path.join(os.path.dirname(FORGE), "dartforge-wasm-run"))
WORK = os.path.join(HERE, "work")
RES = os.path.join(HERE, "results")
TIMEOUT = 30
EXCLUDED_IMPORTS = ("dart:io", "dart:isolate", "dart:mirrors", "dart:ffi", "dart:_", "dart:html", "dart:js")
STATIC_ERROR_MARKERS = ("//#", "// [analyzer", "// [cfe", "// [web]")
PACKAGE_CONFIG = """{"configVersion":2,"packages":[
{"name":"expect","rootUri":"../pkg/expect","packageUri":"lib/","languageVersion":"3.0"},
{"name":"async_helper","rootUri":"../pkg/async_helper","packageUri":"lib/","languageVersion":"3.0"},
{"name":"tests","rootUri":"../tests","packageUri":"lib/","languageVersion":"3.6"}
]}
"""

MAIN_MJS = """process.on('uncaughtException', (e) => {{
  console.error('Unhandled exception:\\n' + e);
  process.exit(255);
}});
import {{ {exp} as m }} from './{mod}.js';
m.main();
"""


def run(cmd, cwd, timeout=TIMEOUT):
    try:
        p = subprocess.run(cmd, cwd=cwd, capture_output=True, timeout=timeout)
        return p.returncode, p.stdout.decode("utf-8", "replace"), p.stderr.decode("utf-8", "replace")
    except subprocess.TimeoutExpired:
        return -999, "", "timeout"


def exported_name(js):
    for line in js.splitlines():
        if line.startswith("export {"):
            inner = line[len("export {"):].rstrip(";").rstrip("}").strip()
            first = inner.split(",")[0].strip()
            return first.split(" as ")[1].strip() if " as " in first else first
    return None


def work_dir(tool, test):
    d = os.path.join(WORK, tool, hashlib.md5(test.encode()).hexdigest()[:12])
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d)
    return d


def vm(test):
    c, o, e = run([DART, "--packages=" + os.path.join(SDK, ".dart_tool/package_config.json"), test], SDK)
    return {"code": c, "out": o, "err": e[-2000:]}


def forge(test):
    d = work_dir("forge", test)
    c, o, e = run([FORGE, "compile-js", test, "-o", d, "--packages", os.path.join(SDK, ".dart_tool/package_config.json")], SDK, 120)
    if c != 0 or not os.path.exists(os.path.join(d, "main.mjs")):
        return {"code": -1000, "out": "", "err": ("[compile] %d " % c + e)[-2000:]}
    c, o, e = run([NODE, "main.mjs"], d)
    return {"code": c, "out": o, "err": e[-2000:]}


def wasm(test):
    d = work_dir("wasm", test)
    module = os.path.join(d, "main.wasm")
    c, o, e = run([FORGE, "compile-wasm", test, "-o", module, "--packages", os.path.join(SDK, ".dart_tool/package_config.json")], SDK, 300)
    if c != 0 or not os.path.exists(module):
        return {"code": -1000, "out": "", "err": ("[compile] %d " % c + o + e)[-2000:]}
    if os.path.exists(os.path.join(d, "main.mjs")):
        c, o, e = run([NODE, os.path.join(ROOT, "runtime", "wasm", "run_node.mjs"), module], d)
    else:
        c, o, e = run([WASM_RUN, module], d)
    return {"code": c, "out": o, "err": e[-2000:]}


def dart2wasm(test):
    d = work_dir("dart2wasm", test)
    module = os.path.join(d, "main.wasm")
    c, o, e = run([DART, "compile", "wasm", test, "-o", module], SDK, 300)
    if c != 0:
        return {"code": -1000, "out": "", "err": ("[compile] %d " % c + o + e)[-2000:]}
    c, o, e = run([NODE, os.path.join(ROOT, "runtime", "wasm", "run_dart2wasm.mjs"), os.path.join(d, "main.mjs"), module], d)
    return {"code": c, "out": o, "err": e[-2000:]}


def by_policy(r):
    """dart2wasm's result read as the VM's where only the message differs
    (crates/differential/src/report.rs, `dart2wasm_by_policy`)."""
    text = lambda t: t.replace("Type '", "type '").replace("JSStringImpl", "String")
    return {"code": r["code"], "out": text(r["out"]), "err": text(r["err"])}


def same(a, b):
    return a["code"] == b["code"] and a["out"] == b["out"]


def fails_like(vm_r, d2w, r):
    """The VM's output up to an uncaught error dart2wasm also reports."""
    msg = lambda x: (x["err"].splitlines()[1:2] or [None])[0]
    return (r["code"] == 255 and d2w["code"] == 255 and r["err"].startswith("Unhandled exception:")
            and msg(r) is not None and msg(r) == msg(d2w) and vm_r["out"].startswith(r["out"]))


def matches(tool, test, r, ref, alt):
    """`vm`, `dart2wasm` (only the web reference), `rejected` (`wasm`: a
    program the VM refuses to compile, 254, that DartForge refuses too), or
    None."""
    if test in ref and same(r, ref[test]):
        return "vm"
    if tool == "wasm" and test in ref and ref[test]["code"] == 254 and r["code"] == -1000:
        return "rejected"
    if tool == "wasm" and test in alt and test in ref:
        a = alt[test]
        if same(r, a) or same(r, by_policy(a)) or fails_like(ref[test], a, r):
            return "dart2wasm"
    return None


def ddc(test):
    d = work_dir("ddc", test)
    mod = "t"
    js = os.path.join(d, mod + ".js")
    c, o, e = run([DART, os.path.join(DART_SDK, "bin/snapshots/dartdevc.dart.snapshot"),
                   "--modules=es6",
                   "--packages=" + os.path.join(SDK, ".dart_tool/package_config.json"),
                   "-o", js, test], SDK, 120)
    if c != 0:
        return {"code": -1000, "out": "", "err": ("[compile] %d " % c + o + e)[-2000:]}
    sdkjs = os.path.join(ROOT, "runtime", "ddc", "dart_sdk.js")
    shutil.copy(sdkjs, os.path.join(d, "dart_sdk.js"))
    src = open(js).read().replace("from 'dart_sdk.js'", "from './dart_sdk.js'")
    open(js, "w").write(src)
    exp = exported_name(src)
    if exp is None:
        return {"code": -1000, "out": "", "err": "no export"}
    open(os.path.join(d, "main.mjs"), "w").write(MAIN_MJS.format(exp=exp, mod=mod))
    c, o, e = run([NODE, "main.mjs"], d)
    return {"code": c, "out": o, "err": e[-2000:]}


def setup(args):
    checkout, tag = None, "3.6.2"
    while args:
        a = args.pop(0)
        if a == "--checkout":
            checkout = args.pop(0)
        elif a == "--tag":
            tag = args.pop(0)
    if not checkout:
        sys.exit("setup needs --checkout <Dart SDK source checkout>")
    shutil.rmtree(SDK, ignore_errors=True)
    os.makedirs(SDK)
    archive = subprocess.run(
        ["git", "-C", checkout, "archive", tag, "tests/language", "pkg/expect", "pkg/async_helper"],
        check=True, capture_output=True).stdout
    tarfile.open(fileobj=io.BytesIO(archive)).extractall(SDK)
    os.makedirs(os.path.join(SDK, ".dart_tool"), exist_ok=True)
    open(os.path.join(SDK, ".dart_tool", "package_config.json"), "w").write(PACKAGE_CONFIG)
    tests = []
    for d, _, files in os.walk(os.path.join(SDK, "tests", "language")):
        for f in files:
            if not f.endswith("_test.dart"):
                continue
            path = os.path.join(d, f)
            rel = os.path.relpath(path, SDK)
            if "/vm/" in rel:
                continue
            text = open(path, encoding="utf-8", errors="replace").read()
            if any(m in text for m in STATIC_ERROR_MARKERS) or any(i in text for i in EXCLUDED_IMPORTS):
                continue
            tests.append(rel)
    tests.sort()
    open(os.path.join(HERE, "list.txt"), "w").write("\n".join(tests) + "\n")
    print("%d programs from %s at %s" % (len(tests), checkout, tag))


def compare(old, new, tool="forge"):
    vm = json.load(open(os.path.join(RES, "vm.json")))
    a = json.load(open(os.path.join(RES, old + ".json")))
    b = json.load(open(os.path.join(RES, new + ".json")))
    alt_path = os.path.join(RES, "dart2wasm.json")
    alt = json.load(open(alt_path)) if tool == "wasm" and os.path.exists(alt_path) else {}
    eq = lambda r, t: t in r and matches(tool, t, r[t], vm, alt) is not None
    print("regressions:", [t for t in vm if eq(a, t) and not eq(b, t)])
    print("fixed:", len([t for t in vm if not eq(a, t) and eq(b, t)]))

def main():
    args = sys.argv[1:]
    tool = args.pop(0)
    if tool == "setup":
        return setup(args)
    if tool == "compare":
        if args[0] == "--wasm":
            return compare(args[1], args[2], "wasm")
        return compare(args[0], args[1])
    jobs = 8
    lst = os.path.join(HERE, "list.txt")
    out = tool
    filters = []
    while args:
        a = args.pop(0)
        if a == "--jobs":
            jobs = int(args.pop(0))
        elif a == "--list":
            lst = args.pop(0)
        elif a == "--out":
            out = args.pop(0)
        else:
            filters.append(a)
    tests = [l.strip() for l in open(lst) if l.strip()]
    if filters:
        tests = [t for t in tests if any(f in t for f in filters)]
    fn = {"vm": vm, "ddc": ddc, "forge": forge, "wasm": wasm, "dart2wasm": dart2wasm}[tool]
    os.makedirs(RES, exist_ok=True)
    path = os.path.join(RES, out + ".json")
    results = json.load(open(path)) if os.path.exists(path) and filters else {}
    ref = json.load(open(os.path.join(RES, "vm.json"))) if tool != "vm" else {}
    alt_path = os.path.join(RES, "dart2wasm.json")
    alt = json.load(open(alt_path)) if tool == "wasm" and os.path.exists(alt_path) else {}
    with cf.ThreadPoolExecutor(jobs) as ex:
        for t, r in zip(tests, ex.map(fn, tests)):
            results[t] = r
            if matches(tool, t, r, ref, alt):
                shutil.rmtree(os.path.join(WORK, tool, hashlib.md5(t.encode()).hexdigest()[:12]), ignore_errors=True)
    json.dump(results, open(path, "w"), indent=0, sort_keys=True)
    if tool != "vm":
        kinds = {t: matches(tool, t, results[t], ref, alt) for t in tests}
        ok = [t for t in tests if kinds[t]]
        bad = [t for t in tests if t not in ok]
        print("%s: %d/%d equal to the VM" % (tool, len([t for t in ok if kinds[t] == "vm"]), len(tests)))
        if tool == "wasm":
            print("%s: %d/%d equal to the VM or to dart2wasm (%d only to dart2wasm, %d rejected by both)" % (
                tool, len(ok), len(tests), len([t for t in ok if kinds[t] == "dart2wasm"]),
                len([t for t in ok if kinds[t] == "rejected"])))
        with open(os.path.join(RES, out + ".fail.txt"), "w") as f:
            for t in bad:
                f.write(t + "\n")
        if filters:
            for t in bad:
                print("FAIL", t, results[t]["code"], ref.get(t, {}).get("code"), (results[t]["err"] or "").strip().splitlines()[:1])

if __name__ == "__main__":
    main()
