#!/usr/bin/env python3
"""Runs the Dart SDK's tests/language suite on the VM, DDC, dart2js, dart2wasm or DartForge
(JS, WebAssembly or native AOT).

The suite is the one of a Dart SDK source checkout at a tag (3.6.2 by
default), extracted into a work directory with `setup`; the programs are the
`*_test.dart` files without static-error expectations (no `//#` multitest
markers, `// [analyzer]`, `// [cfe]` or `// [web]` annotations), outside
`vm/`, that import no `dart:io`, `dart:isolate`, `dart:mirrors`, `dart:ffi`,
`dart:html`, `dart:js` or private `dart:_` library.

usage:
  run-language-suite.py setup --checkout DIR [--tag 3.6.2] [--meta DIR] [--collection DIR]
  run-language-suite.py setup --packages-only [--checkout DIR] [--tag 3.6.2] [--meta DIR] [--collection DIR]
  run-language-suite.py vm|dartaot|ddc|dart2js|dart2wasm|forge|jsprod|wasm|native|jit [--jobs N] [--out NAME]
        [--list FILE] [--ref NAME] [--alt NAME] [--js-level -O1|-O2]
        [--stack-trace none|symbolic] [filter...]
  run-language-suite.py compare [--forge|--native|--wasm|--dartaot] [--ref NAME] [--alt NAME] OLD NEW
        (by result name: regressions, fixes and the moves between "equal" and
         "rejected by both"; --wasm also accepts dart2wasm's results)

A tool's results go to <work>/results/<name>.json as {test: {code, out, err}};
the other tools compare against the VM's reference (`--ref`, default
`vm.json`; `--alt`, dart2wasm's for `wasm`, default `dart2wasm.json`) and
write <name>.fail.txt. With filters or `--list` only those programs run and
the results merge into an existing <name>.json. Each invocation writes a
<name>.journal.jsonl and <name>.run.json with its selected and completed counts;
the result snapshot is checkpointed every 50 completions. Failures return exit 1,
including VM compile/runtime failures; empty selections and invalid options are errors.
Stdout and stderr tails are retained. Identity hashes are not normalized.
An exit-zero asynchronous test must print `unittest-suite-success` after
requesting completion with `unittest-suite-wait-for-done`, as required by
the SDK test runner. An incomplete test fails without changing its raw status.

`jsprod` runs the separate production compiler (`JSPROD`, default
target/release/dartforge-jsprod), using its own SDK and checks enabled. Both
`jsprod` and `dart2js` use `--js-level` (default -O2; only -O1 and -O2 are
accepted). A successful result equal to dart2js, but different from the VM,
is counted separately. A shared runtime failure is not accepted by that rule.
`native --stack-trace symbolic` explicitly enables symbolic traces; without
the option the CLI/environment default applies. Record profiles separately.

Packages: the suite's `.dart_tool/package_config.json` has `expect`,
`async_helper` and `tests`, and also `meta` (`pkg/meta`: the three
`type_promotion/closure_runtime*` tests) and `collection`
(`regress/regress14348`), at the versions the 3.6.2 SDK pins (PINNED below:
its `pkg/meta` is meta 1.16.0, and its DEPS `collection_rev` 24b75d85 is
collection 1.19.0). They come from the pub cache
(`$PUB_CACHE/hosted/pub.dev/<name>-<version>`) when it has them, else from the
pub.dev archive (`https://pub.dev/api/archives/<name>-<version>.tar.gz`);
`--meta DIR` and `--collection DIR` copy a package from a directory instead.
`--packages-only` adds the two packages and rewrites the package config
without extracting the suite again. A package's language version is the lower
bound of its pubspec's `sdk:` constraint, as pub writes it.

`jit` runs the program in DartForge's JIT (`dartforge run`, the LLVM ORC JIT
of the same native lowering, built with `--features jit`), with the test's
experiments, counted like `native`.

`dartaot` is the official AOT compiler: `dart compile exe` (with the test's
experiments), then the executable; a reference measurement, counted against
the VM like the DartForge tools.

Experiments: a test's `// SharedOptions=` and `// VMOptions=` lines may ask
for `--enable-experiment=a,b` (`variance`, `wildcard-variables`, `macros`…).
Every tool gets them: the VM (`dart --enable-experiment=…`), DDC, dart2wasm and
DartForge (`--language-version=3.6 --enable-experiment=…`, the per-program
language flags of `compile-js`, `compile-wasm` and `aot`: an experiment only
applies to a library at the tool's current version, and the suite's package
is at 3.6, the 3.6.2 VM's current version). An experiment DartForge does not
know is refused at the command line with a diagnostic that names it, so it
shows as a compile failure (a real, visible gap). Other VM options
(optimization thresholds, `-D` defines) are not passed.

Counting, for every tool other than `vm`, each program is exactly one of:
  - equal: same exit code and same stdout as the VM (`vm.json`);
  - only dart2wasm (`wasm` only): equal to dart2wasm where the web platform
    makes it differ from the VM (cast messages read as the VM's, or the VM's
    output up to an uncaught error with dart2wasm's message);
  - DDC's divergence (`forge` only): DDC (`ddc.json`, `--alt`) differs from
    the VM on the program and the JS dev result departs from the VM the same
    way — DDC's exit code, and DDC's stdout or the first line that differs
    from the VM's at the same place (the closure's JS source after `from:`,
    the JS stack frames and the hashes there come from each one's generated
    code). The web platform, not a bug of ours; it is not a failure;
  - rejected by both: the VM refuses to compile it (exit 254) and DartForge
    refuses it too (compile failure, -1000) — a program with a static error
    the runner did not filter out; it is not a failure;
  - passes where the VM fails: the VM fails one of the test's `Expect` checks
    (255, `Unhandled exception:` then `Expect…`) and DartForge runs the whole
    test (exit 0, empty stderr, the VM's stdout up to its failure) — the 3.6.2
    VM does not implement what the test checks (the subtyping of the
    `variance` experiment); it is not a failure, and the list is printed;
  - failure: anything else (`<name>.fail.txt`), including a program the VM
    refuses and DartForge accepts.
The summary prints the numbers separately.

`native` compiles with `dartforge aot --optimize` (as `dart compile exe`: asserts
off, like the VM run) and runs the executable; a program the VM refuses (254)
that DartForge refuses too counts as rejected by both, as in `wasm`.

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
import json, os, re, subprocess, sys, hashlib, shutil, tarfile, io, time, concurrent.futures as cf

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
JSPROD = os.environ.get("JSPROD", os.path.join(ROOT, "target", "release", "dartforge-jsprod"))
JS_LEVEL = "-O2"
STACK_TRACE = None
WASM_RUN = os.environ.get("WASM_RUN", os.path.join(os.path.dirname(FORGE), "dartforge-wasm-run"))
WORK = os.path.join(HERE, "work")
RES = os.path.join(HERE, "results")
TIMEOUT = 30
EXCLUDED_IMPORTS = ("dart:io", "dart:isolate", "dart:mirrors", "dart:ffi", "dart:_", "dart:html", "dart:js")
STATIC_ERROR_MARKERS = ("//#", "// [analyzer", "// [cfe", "// [web]")
# The packages of the suite: name, directory under the work SDK, and the
# language version when the directory has no pubspec to read it from.
PACKAGES = (
    ("expect", "pkg/expect", "3.0"),
    ("async_helper", "pkg/async_helper", "3.0"),
    ("meta", "pkg/meta", None),
    ("collection", "third_party/pkg/collection", None),
    ("tests", "tests", "3.6"),
)


def pubspec_language_version(directory):
    """The lower bound of the pubspec's `sdk:` constraint as `x.y` (`^3.4.0`,
    `>=2.12.0 <4.0.0`), or None."""
    try:
        text = open(os.path.join(directory, "pubspec.yaml"), encoding="utf-8").read()
    except OSError:
        return None
    m = re.search(r"^\s+sdk:\s*['\"]?(?:\^|>=)\s*(\d+)\.(\d+)", text, re.M)
    return "%s.%s" % (m.group(1), m.group(2)) if m else None


def package_config(sdk):
    """The package config of the work SDK: every package of PACKAGES whose
    directory exists (`expect`, `async_helper` and `tests` keep the language
    versions they always had)."""
    entries = []
    for name, rel, version in PACKAGES:
        directory = os.path.join(sdk, rel)
        if not os.path.isdir(directory):
            continue
        version = version or pubspec_language_version(directory) or "3.0"
        entries.append('{"name":"%s","rootUri":"../%s","packageUri":"lib/","languageVersion":"%s"}'
                       % (name, rel, version))
    return '{"configVersion":2,"packages":[\n' + ",\n".join(entries) + "\n]}\n"

MAIN_MJS = """process.on('uncaughtException', (e) => {{
  console.error('Unhandled exception:\\n' + e);
  process.exit(255);
}});
import {{ {exp} as m }} from './{mod}.js';
m.main();
"""


def experiments(test):
    """The `--enable-experiment=` names the test asks for in its
    `// SharedOptions=` and `// VMOptions=` lines, in order."""
    names = []
    try:
        text = open(os.path.join(SDK, test), encoding="utf-8", errors="replace").read()
    except OSError:
        return names
    for line in text.splitlines():
        line = line.strip()
        for key in ("// SharedOptions=", "// VMOptions="):
            if line.startswith(key):
                for opt in line[len(key):].split():
                    if opt.startswith("--enable-experiment="):
                        for n in opt[len("--enable-experiment="):].split(","):
                            if n and n not in names:
                                names.append(n)
    return names


def experiment_flags(test):
    names = experiments(test)
    return ["--enable-experiment=" + ",".join(names)] if names else []


def forge_experiment_flags(test):
    """DartForge's flags for the test's experiments. An experiment only
    applies to a library whose default version is the tool's current one, and
    the suite's package is at 3.6, the current version of the 3.6.2 VM it is
    compared against: DartForge (current 3.13) gets `--language-version=3.6`
    with them, so the experiments apply as in the VM."""
    flags = experiment_flags(test)
    return ["--language-version=3.6"] + flags if flags else []


def run(cmd, cwd, timeout=TIMEOUT):
    try:
        p = subprocess.run(cmd, cwd=cwd, capture_output=True, timeout=timeout)
        # The Windows runs (`vm-win*.json`) are compared with `\n` endings.
        text = lambda b: b.decode("utf-8", "replace").replace("\r\n", "\n")
        return p.returncode, text(p.stdout), text(p.stderr)
    except subprocess.TimeoutExpired:
        return -999, "", "timeout"


def compile_failure(code, out, err):
    """Retain the actual compiler status, including crashes and timeouts."""
    return {"code": -1000, "compile_code": code, "out": "",
            "err": ("[compile] %d " % code + out + err)[-2000:]}


JS_PRELUDE = ("globalThis.self = Object.create(globalThis);\n"
              "self.scheduleImmediate = (f) => { Promise.resolve().then(f); };\n"
              "require('./main.js');\n")


def production_js(test, official=False):
    """Compile and run both production backends under the same Node host."""
    d = work_dir("dart2js" if official else "jsprod", test)
    js = os.path.join(d, "main.js")
    packages = os.path.join(SDK, ".dart_tool/package_config.json")
    if official:
        cmd = [DART, "compile", "js", JS_LEVEL, "--packages=" + packages]
        cmd += experiment_flags(test) + [test, "-o", js]
    else:
        cmd = [JSPROD, test, "-o", js, JS_LEVEL, "--own-sdk", "--sdk",
               os.path.join(DART_SDK, "lib"), "--packages", packages]
        cmd += forge_experiment_flags(test)
    c, o, e = run(cmd, SDK, 300)
    if c != 0 or not os.path.exists(js):
        return compile_failure(c, o, e)
    # CJS keeps browser shims out of generated source and supports async tests.
    with open(os.path.join(d, "run.cjs"), "w", encoding="utf-8") as f:
        f.write(JS_PRELUDE)
    c, o, e = run([NODE, "run.cjs"], d)
    return {"code": c, "out": o, "err": e[-2000:]}


def dart2js(test):
    return production_js(test, official=True)


def jsprod(test):
    return production_js(test)


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
    c, o, e = run([DART] + experiment_flags(test) + ["--packages=" + os.path.join(SDK, ".dart_tool/package_config.json"), test], SDK)
    return {"code": c, "out": o, "err": e[-2000:]}


def dartaot(test):
    # The official AOT compiler, as the Windows measurement's runner does it.
    d = work_dir("dartaot", test)
    exe = os.path.join(d, "main.exe")
    c, o, e = run([DART, "compile", "exe"] + experiment_flags(test)
                  + ["--packages=" + os.path.join(SDK, ".dart_tool/package_config.json"), test, "-o", exe], SDK, 300)
    if c != 0 or not os.path.exists(exe):
        return compile_failure(c, o, e)
    c, o, e = run([exe], d)
    return {"code": c, "out": o, "err": e[-2000:]}


def forge(test):
    d = work_dir("forge", test)
    c, o, e = run([FORGE, "compile-js", test, "-o", d, "--packages", os.path.join(SDK, ".dart_tool/package_config.json")] + forge_experiment_flags(test), SDK, 120)
    if c != 0 or not os.path.exists(os.path.join(d, "main.mjs")):
        return compile_failure(c, o, e)
    c, o, e = run([NODE, "main.mjs"], d)
    return {"code": c, "out": o, "err": e[-2000:]}


def wasm(test):
    d = work_dir("wasm", test)
    module = os.path.join(d, "main.wasm")
    c, o, e = run([FORGE, "compile-wasm", test, "-o", module, "--packages", os.path.join(SDK, ".dart_tool/package_config.json")] + forge_experiment_flags(test), SDK, 300)
    if c != 0 or not os.path.exists(module):
        return compile_failure(c, o, e)
    if os.path.exists(os.path.join(d, "main.mjs")):
        c, o, e = run([NODE, os.path.join(ROOT, "runtime", "wasm", "run_node.mjs"), module], d)
    else:
        c, o, e = run([WASM_RUN, module], d)
    return {"code": c, "out": o, "err": e[-2000:]}


def jit(test):
    # `dartforge run`: the native lowering in the JIT, as `dart run` (asserts
    # off, like the VM run of vm.json).
    c, o, e = run([FORGE, "run", "--packages", os.path.join(SDK, ".dart_tool/package_config.json")]
                  + forge_experiment_flags(test) + [test], SDK, 300)
    return {"code": c, "out": o, "err": e[-2000:]}


def native(test):
    # As `dart compile exe`: asserts off, like the VM run of vm.json.
    d = work_dir("native", test)
    exe = os.path.join(d, "main.exe" if sys.platform == "win32" else "main")
    flags = ["--stack-trace", STACK_TRACE] if STACK_TRACE is not None else []
    c, o, e = run([FORGE, "aot", test, exe, "--optimize", "--packages", os.path.join(SDK, ".dart_tool/package_config.json")] + flags + forge_experiment_flags(test), SDK, 300)
    if c != 0 or not os.path.exists(exe):
        return compile_failure(c, o, e)
    c, o, e = run([exe], d)
    return {"code": c, "out": o, "err": e[-2000:]}


def dart2wasm(test):
    d = work_dir("dart2wasm", test)
    module = os.path.join(d, "main.wasm")
    c, o, e = run([DART, "compile", "wasm"] + experiment_flags(test) + [test, "-o", module], SDK, 300)
    if c != 0:
        return compile_failure(c, o, e)
    c, o, e = run([NODE, os.path.join(ROOT, "runtime", "wasm", "run_dart2wasm.mjs"), os.path.join(d, "main.mjs"), module], d)
    return {"code": c, "out": o, "err": e[-2000:]}


def by_policy(r):
    """dart2wasm's result read as the VM's where only the message differs
    (crates/differential/src/report.rs, `dart2wasm_by_policy`)."""
    text = lambda t: t.replace("Type '", "type '").replace("JSStringImpl", "String")
    return {"code": r["code"], "out": text(r["out"]), "err": text(r["err"])}


def same(a, b):
    return a["code"] == b["code"] and a["out"] == b["out"]


def incomplete_async(r):
    """Detect exit-zero tests missing the SDK async-helper completion signal."""
    return (r["code"] == 0 and "unittest-suite-wait-for-done" in r["out"]
            and "unittest-suite-success" not in r["out"])


def fails_like(vm_r, d2w, r):
    """The VM's output up to an uncaught error dart2wasm also reports."""
    msg = lambda x: (x["err"].splitlines()[1:2] or [None])[0]
    return (r["code"] == 255 and d2w["code"] == 255 and r["err"].startswith("Unhandled exception:")
            and msg(r) is not None and msg(r) == msg(d2w) and vm_r["out"].startswith(r["out"]))


def default_alt(tool):
    """The web reference of a tool: dart2wasm for `wasm`, DDC for `forge`."""
    return {"wasm": "dart2wasm", "forge": "ddc", "jsprod": "dart2js"}.get(tool, "dart2wasm")


def first_difference(a, b):
    """The index of the first stdout line where `a` and `b` differ, or None."""
    x, y = a.splitlines(), b.splitlines()
    for k in range(max(len(x), len(y))):
        if (x[k] if k < len(x) else None) != (y[k] if k < len(y) else None):
            return k
    return None


def same_divergence(vm_r, ddc_r, r):
    """`r` departs from the VM where DDC does, the same way: DDC differs from
    the VM, and `r` has DDC's exit code, the same stdout or the first stdout
    line that differs from the VM's at the same place (the closure's JS source
    after `from:`, the JS stack frames and the hashes of that line may
    differ; DDC prints them from its own generated code)."""
    if same(ddc_r, vm_r) or r["code"] != ddc_r["code"]:
        return False
    if r["out"] == ddc_r["out"]:
        return True
    return first_difference(vm_r["out"], r["out"]) == first_difference(vm_r["out"], ddc_r["out"])


def vm_fails_expectation(vm_r, r):
    """The VM fails one of the test's `Expect` checks (255, the expectation's
    message) and `r` runs the whole test: exit 0, nothing on stderr, and the
    VM's stdout up to its failure."""
    lines = vm_r["err"].splitlines()
    return (vm_r["code"] == 255 and len(lines) > 1 and lines[0].startswith("Unhandled exception:")
            and lines[1].startswith("Expect") and r["code"] == 0 and not r["err"].strip()
            and r["out"].startswith(vm_r["out"]))


def matches(tool, test, r, ref, alt):
    """`vm`, `dart2wasm` (`wasm`: only the web reference), `ddc` (`forge`: the
    same divergence from the VM as DDC's), `rejected` (a program the VM refuses
    to compile, 254, that the tool refuses too: `forge`, `wasm`, `native` and
    `dartaot`), `passes` (the test passes where the VM fails one of its
    expectations), or None (a failure)."""
    if incomplete_async(r):
        return None
    if r["code"] < 0 or r.get("compile_code", 1) <= 0:
        # A compile refusal is considered below; infrastructure failures never match.
        if r["code"] != -1000 or r.get("compile_code", 1) <= 0:
            return None
    # A successful dart2js reference cannot be matched by a failing VM run.
    # Shared VM failures are not evidence for the production JS contract.
    if tool == "jsprod" and test in alt and alt[test]["code"] == 0 and r["code"] != 0:
        return None
    if test in ref and ref[test]["code"] >= 0 and same(r, ref[test]):
        return "vm"
    if test in ref and vm_fails_expectation(ref[test], r):
        return "passes"
    if tool in ("forge", "wasm", "native", "dartaot", "dart2js", "jsprod") and test in ref and ref[test]["code"] == 254 and r["code"] == -1000 and r.get("compile_code", 1) != 101:
        return "rejected"
    # `jit` compiles and runs in one process: a refusal exits like a compile
    # error of `dartforge` (1) with no output, after the VM refused too.
    if tool == "jit" and test in ref and ref[test]["code"] == 254 and r["code"] == 1 and not r["out"]:
        return "rejected"
    if tool == "wasm" and test in alt and test in ref:
        a = alt[test]
        if same(r, a) or same(r, by_policy(a)) or fails_like(ref[test], a, r):
            return "dart2wasm"
    if tool == "forge" and test in alt and test in ref and same_divergence(ref[test], alt[test], r):
        return "ddc"
    if tool == "jsprod" and test in alt and test in ref and r["code"] == 0 and not r["err"].strip() and same(r, alt[test]) and not alt[test]["err"].strip():
        return "dart2js"
    return None


def ddc(test):
    d = work_dir("ddc", test)
    mod = "t"
    js = os.path.join(d, mod + ".js")
    c, o, e = run([DART, os.path.join(DART_SDK, "bin/snapshots/dartdevc.dart.snapshot"),
                   "--modules=es6",
                   "--packages=" + os.path.join(SDK, ".dart_tool/package_config.json")]
                  + experiment_flags(test) + ["-o", js, test], SDK, 120)
    if c != 0:
        return compile_failure(c, o, e)
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


def copy_package(source, rel):
    """`source`'s `lib/` and pubspec into `rel` of the work SDK."""
    target = os.path.join(SDK, rel)
    shutil.rmtree(target, ignore_errors=True)
    shutil.copytree(os.path.join(source, "lib"), os.path.join(target, "lib"))
    if os.path.exists(os.path.join(source, "pubspec.yaml")):
        shutil.copy(os.path.join(source, "pubspec.yaml"), target)


# The versions of the packages the 3.6.2 SDK pins (see Packages above).
PINNED = {"meta": ("1.16.0", "pkg/meta"), "collection": ("1.19.0", "third_party/pkg/collection")}


def pub_cache():
    if os.environ.get("PUB_CACHE"):
        return os.environ["PUB_CACHE"]
    if sys.platform == "win32":
        return os.path.join(os.environ.get("LOCALAPPDATA", ""), "Pub", "Cache")
    return os.path.join(os.path.expanduser("~"), ".pub-cache")


def pinned_package(name):
    """The pinned version of `name`: its pub cache directory, or the pub.dev
    archive extracted into the work SDK's `.dart_tool/archives`."""
    version, _ = PINNED[name]
    cached = os.path.join(pub_cache(), "hosted", "pub.dev", "%s-%s" % (name, version))
    if os.path.isdir(os.path.join(cached, "lib")):
        return cached
    import urllib.request
    url = "https://pub.dev/api/archives/%s-%s.tar.gz" % (name, version)
    target = os.path.join(SDK, ".dart_tool", "archives", "%s-%s" % (name, version))
    shutil.rmtree(target, ignore_errors=True)
    os.makedirs(target)
    data = urllib.request.urlopen(url, timeout=60).read()
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as tar:
        for member in tar.getmembers():
            # Only regular files and directories, inside the target.
            path = os.path.normpath(member.name)
            if path.startswith("..") or os.path.isabs(path) or not (member.isfile() or member.isdir()):
                continue
            tar.extract(member, target)
    return target


def install_packages(checkout, tag, meta, collection):
    """`meta` and `collection` into the work SDK (see Packages above), and the
    package config."""
    copy_package(meta or pinned_package("meta"), PINNED["meta"][1])
    copy_package(collection or pinned_package("collection"), PINNED["collection"][1])
    for name, rel in (("meta", "pkg/meta"), ("collection", "third_party/pkg/collection")):
        if not os.path.isdir(os.path.join(SDK, rel)):
            print("warning: no `%s` package (%s): the programs that import it are refused" % (name, rel))
    os.makedirs(os.path.join(SDK, ".dart_tool"), exist_ok=True)
    open(os.path.join(SDK, ".dart_tool", "package_config.json"), "w").write(package_config(SDK))


def setup(args):
    checkout, tag, meta, collection, only = None, "3.6.2", None, None, False
    while args:
        a = args.pop(0)
        if a == "--checkout":
            checkout = args.pop(0)
        elif a == "--tag":
            tag = args.pop(0)
        elif a == "--meta":
            meta = args.pop(0)
        elif a == "--collection":
            collection = args.pop(0)
        elif a == "--packages-only":
            only = True
    if only:
        return install_packages(checkout, tag, meta, collection)
    if not checkout:
        sys.exit("setup needs --checkout <Dart SDK source checkout>")
    shutil.rmtree(SDK, ignore_errors=True)
    os.makedirs(SDK)
    archive = subprocess.run(
        ["git", "-C", checkout, "archive", tag, "tests/language", "pkg/expect", "pkg/async_helper"],
        check=True, capture_output=True).stdout
    tarfile.open(fileobj=io.BytesIO(archive)).extractall(SDK)
    install_packages(checkout, tag, meta, collection)
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


def kind(tool, test, results, ref, alt):
    """`equal` (the VM, or dart2wasm for `wasm`), `ddc` (`forge`: DDC's
    divergence), `rejected` (by both), `failure`, or None when `results` lacks
    the program."""
    if test not in results:
        return None
    m = matches(tool, test, results[test], ref, alt)
    if m is None:
        return "failure"
    return m if m in ("rejected", "ddc", "passes") else "equal"


def compare(old, new, tool="forge", ref_name="vm", alt_name=None):
    """Each program present in both result files, by its kind in each: a
    regression goes from equal or rejected to a failure, a fix the other way;
    a move between equal and rejected by both (the VM's reference or a
    refusal changed) is listed apart."""
    vm = json.load(open(os.path.join(RES, ref_name + ".json")))
    a = json.load(open(os.path.join(RES, old + ".json")))
    b = json.load(open(os.path.join(RES, new + ".json")))
    alt_path = os.path.join(RES, (alt_name or default_alt(tool)) + ".json")
    alt = json.load(open(alt_path)) if tool in ("wasm", "forge", "jsprod") and os.path.exists(alt_path) else {}
    moves = {}
    for t in vm:
        ka, kb = kind(tool, t, a, vm, alt), kind(tool, t, b, vm, alt)
        if ka is None or kb is None or ka == kb:
            continue
        moves.setdefault((ka, kb), []).append(t)
    print("regressions:", sorted(moves.get(("equal", "failure"), []) + moves.get(("rejected", "failure"), [])
                                 + moves.get(("ddc", "failure"), []) + moves.get(("passes", "failure"), [])))
    n = lambda k: len(moves.get(("failure", k), []))
    print("fixed:", n("equal") + n("rejected") + n("ddc") + n("passes"),
          "(%d now equal, %d now rejected by both, %d now DDC's divergence, %d now passing where the VM fails)" % (
              n("equal"), n("rejected"), n("ddc"), n("passes")))
    for (ka, kb), ts in sorted(moves.items()):
        if "failure" not in (ka, kb):
            print("%s -> %s: %s" % (ka, kb, sorted(ts)))

def write_json(path, value):
    """Replace a checkpoint only after its complete JSON has been written."""
    temporary = path + ".tmp"
    with open(temporary, "w", encoding="utf-8") as f:
        json.dump(value, f, indent=0, sort_keys=True)
    os.replace(temporary, path)


def read_json(path):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def file_hash(path):
    """Hash an available input without requiring unrelated backend tools."""
    if not os.path.isfile(path):
        return None
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def main(argv=None):
    global JS_LEVEL, STACK_TRACE
    JS_LEVEL, STACK_TRACE = "-O2", None
    args = list(sys.argv[1:] if argv is None else argv)
    if not args:
        raise ValueError("expected a tool or setup/compare")
    tool = args.pop(0)
    if tool == "setup":
        return setup(args)
    ref_name, alt_name = "vm", None
    rest = []
    while args:
        a = args.pop(0)
        if a == "--ref":
            ref_name = args.pop(0)
        elif a == "--alt":
            alt_name = args.pop(0)
        else:
            rest.append(a)
    args = rest
    if tool == "compare":
        kind_ = "forge"
        if args[0] in ("--forge", "--native", "--wasm", "--dartaot", "--jit", "--jsprod", "--dart2js"):
            kind_ = args.pop(0)[2:]
        return compare(args[0], args[1], kind_, ref_name, alt_name)
    jobs = 8
    lst = os.path.join(HERE, "list.txt")
    listed = False
    out = tool
    filters = []
    while args:
        a = args.pop(0)
        if a == "--jobs":
            jobs = int(args.pop(0))
        elif a == "--list":
            lst = args.pop(0)
            listed = True
        elif a == "--out":
            out = args.pop(0)
        elif a == "--js-level":
            JS_LEVEL = args.pop(0)
            if JS_LEVEL not in ("-O1", "-O2") or tool not in ("jsprod", "dart2js"):
                raise ValueError("--js-level requires jsprod/dart2js and -O1 or -O2")
        elif a == "--stack-trace":
            STACK_TRACE = args.pop(0)
            if STACK_TRACE not in ("none", "symbolic") or tool != "native":
                raise ValueError("--stack-trace requires native and none or symbolic")
        elif a.startswith("-"):
            raise ValueError("unknown option: " + a)
        else:
            filters.append(a)
    with open(lst, encoding="utf-8") as f:
        tests = [l.strip() for l in f if l.strip()]
    if filters:
        tests = [t for t in tests if any(f in t for f in filters)]
    if jobs < 1 or not tests or len(tests) != len(set(tests)):
        raise ValueError("expected positive jobs and a nonempty list of unique tests")
    fn = {"vm": vm, "dartaot": dartaot, "ddc": ddc, "forge": forge, "wasm": wasm, "native": native,
          "dart2wasm": dart2wasm, "jit": jit, "dart2js": dart2js, "jsprod": jsprod}[tool]
    os.makedirs(RES, exist_ok=True)
    path = os.path.join(RES, out + ".json")
    results = read_json(path) if os.path.exists(path) and (filters or listed) else {}
    ref = read_json(os.path.join(RES, ref_name + ".json")) if tool != "vm" else {}
    alt_path = os.path.join(RES, (alt_name or default_alt(tool)) + ".json")
    alt = read_json(alt_path) if tool in ("wasm", "forge", "jsprod") and os.path.exists(alt_path) else {}
    if tool != "vm" and any(t not in ref for t in tests):
        raise ValueError("the VM reference does not cover the selected tests")
    if tool == "jsprod" and any(t not in alt for t in tests):
        raise ValueError("jsprod requires a complete dart2js reference (--alt)")
    profile = {"tool": tool, "jsLevel": JS_LEVEL if tool in ("jsprod", "dart2js") else None,
               "stackTrace": STACK_TRACE or os.environ.get("DARTFORGE_STACK_TRACE") if tool == "native" else None,
               "dart": DART, "dartHash": file_hash(DART),
               "compiler": JSPROD if tool == "jsprod" else FORGE,
               "compilerHash": file_hash(JSPROD if tool == "jsprod" else FORGE),
               "node": NODE, "nodeHash": file_hash(shutil.which(NODE) or NODE),
               "runnerHash": file_hash(__file__),
               "packagesHash": file_hash(os.path.join(SDK, ".dart_tool/package_config.json")),
               "referenceHash": file_hash(os.path.join(RES, ref_name + ".json")) if tool != "vm" else None,
               "alternateHash": file_hash(alt_path) if alt else None}
    if tool == "jsprod":
        alt_status_path = os.path.join(RES, (alt_name or default_alt(tool)) + ".run.json")
        alt_status = read_json(alt_status_path) if os.path.exists(alt_status_path) else {}
        alt_profile = alt_status.get("profile", {})
        fields = ("jsLevel", "dartHash", "nodeHash", "packagesHash")
        if (alt_status.get("state") != "complete" or alt_profile.get("tool") != "dart2js"
                or any(alt_profile.get(key) != profile[key] for key in fields)
                or alt_status.get("resultHash") != file_hash(alt_path)):
            raise ValueError("dart2js reference must be complete and use the same SDK, host and JS level")
    status_path = os.path.join(RES, out + ".run.json")
    if results:
        previous = read_json(status_path) if os.path.exists(status_path) else {}
        if previous.get("profile") != profile:
            raise ValueError("refusing to merge different or unrecorded profiles; choose a new --out")
    status = {"state": "running", "profile": profile, "selected": tests, "completed": [],
              "started": time.time(), "jobs": jobs}
    write_json(status_path, status)
    try:
        with open(os.path.join(RES, out + ".journal.jsonl"), "w", encoding="utf-8") as journal:
            with cf.ThreadPoolExecutor(jobs) as ex:
                pending = {ex.submit(fn, t): t for t in tests}
                for future in cf.as_completed(pending):
                    t = pending[future]
                    r = future.result()
                    results[t] = r
                    status["completed"].append(t)
                    journal.write(json.dumps({"test": t, "result": r}) + "\n")
                    journal.flush()
                    if matches(tool, t, r, ref, alt):
                        shutil.rmtree(os.path.join(WORK, tool, hashlib.md5(t.encode()).hexdigest()[:12]), ignore_errors=True)
                    if len(status["completed"]) % 50 == 0:
                        write_json(path, results)
                        write_json(status_path, status)
                        print("%s: %d/%d completed" % (tool, len(status["completed"]), len(tests)), flush=True)
    except BaseException:
        status["state"] = "interrupted"
        write_json(path, results)
        write_json(status_path, status)
        raise
    write_json(path, results)
    bad = [t for t in tests if results[t]["code"] != 0 or incomplete_async(results[t])] if tool == "vm" else []
    if tool != "vm":
        kinds = {t: matches(tool, t, results[t], ref, alt) for t in tests}
        ok = [t for t in tests if kinds[t]]
        bad = [t for t in tests if t not in ok]
        count = lambda k: len([t for t in ok if kinds[t] == k])
        print("%s: %d/%d equal to the VM" % (tool, count("vm"), len(tests)))
        if tool == "wasm":
            print("%s: %d only equal to dart2wasm" % (tool, count("dart2wasm")))
        if tool == "forge":
            print("%s: %d with DDC's own divergence from the VM" % (tool, count("ddc")))
        if tool == "jsprod":
            print("%s: %d only equal to successful dart2js" % (tool, count("dart2js")))
        if tool in ("forge", "wasm", "native", "dartaot", "jit", "jsprod", "dart2js"):
            print("%s: %d rejected by both (the VM refuses them too)" % (tool, count("rejected")))
        print("%s: %d passing where the VM fails an expectation" % (tool, count("passes")))
        for t in tests:
            if kinds[t] == "passes":
                print("  passes:", t)
        print("%s: %d failures" % (tool, len(bad)))
        with open(os.path.join(RES, out + ".fail.txt"), "w") as f:
            for t in bad:
                f.write(t + "\n")
        if filters:
            for t in bad:
                print("FAIL", t, results[t]["code"], ref.get(t, {}).get("code"), (results[t]["err"] or "").strip().splitlines()[:1])
    else:
        print("vm: %d/%d complete successfully; %d compile/runtime/protocol failures" % (len(tests) - len(bad), len(tests), len(bad)))
    status.update(state="complete", finished=time.time(), failures=bad, exitCode=1 if bad else 0,
                  resultHash=file_hash(path))
    write_json(status_path, status)
    return status["exitCode"]

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, IndexError, KeyError) as error:
        sys.exit("invalid invocation: " + str(error))
