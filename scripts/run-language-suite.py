#!/usr/bin/env python3
"""Runs the Dart SDK's tests/language suite on the VM, DDC, dart2wasm or DartForge
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
  run-language-suite.py vm|dartaot|ddc|dart2wasm|forge|wasm|native|jit [--jobs N] [--out NAME]
        [--list FILE] [--ref NAME] [--alt NAME] [filter...]
  run-language-suite.py compare [--forge|--native|--wasm|--dartaot] [--ref NAME] [--alt NAME] OLD NEW
        (by result name: regressions, fixes and the moves between "equal" and
         "rejected by both"; --wasm also accepts dart2wasm's results)

A tool's results go to <work>/results/<name>.json as {test: {code, out, err}};
the other tools compare against the VM's reference (`--ref`, default
`vm.json`; `--alt`, dart2wasm's for `wasm`, default `dart2wasm.json`) and
write <name>.fail.txt. With filters or `--list` only those programs run and
the results merge into an existing <name>.json.

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
  - rejected by both: the VM refuses to compile it (exit 254) and DartForge
    refuses it too (compile failure, -1000) — a program with a static error
    the runner did not filter out; it is not a failure;
  - failure: anything else (`<name>.fail.txt`), including a program the VM
    refuses and DartForge accepts.
The summary prints the four numbers separately.

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
import json, os, re, subprocess, sys, hashlib, shutil, tarfile, io, concurrent.futures as cf

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
    c, o, e = run([DART] + experiment_flags(test) + ["--packages=" + os.path.join(SDK, ".dart_tool/package_config.json"), test], SDK)
    return {"code": c, "out": o, "err": e[-2000:]}


def dartaot(test):
    # The official AOT compiler, as the Windows measurement's runner does it.
    d = work_dir("dartaot", test)
    exe = os.path.join(d, "main.exe")
    c, o, e = run([DART, "compile", "exe"] + experiment_flags(test)
                  + ["--packages=" + os.path.join(SDK, ".dart_tool/package_config.json"), test, "-o", exe], SDK, 300)
    if c != 0 or not os.path.exists(exe):
        return {"code": -1000, "out": "", "err": ("[compile] %d " % c + o + e)[-2000:]}
    c, o, e = run([exe], d)
    return {"code": c, "out": o, "err": e[-2000:]}


def forge(test):
    d = work_dir("forge", test)
    c, o, e = run([FORGE, "compile-js", test, "-o", d, "--packages", os.path.join(SDK, ".dart_tool/package_config.json")] + forge_experiment_flags(test), SDK, 120)
    if c != 0 or not os.path.exists(os.path.join(d, "main.mjs")):
        return {"code": -1000, "out": "", "err": ("[compile] %d " % c + e)[-2000:]}
    c, o, e = run([NODE, "main.mjs"], d)
    return {"code": c, "out": o, "err": e[-2000:]}


def wasm(test):
    d = work_dir("wasm", test)
    module = os.path.join(d, "main.wasm")
    c, o, e = run([FORGE, "compile-wasm", test, "-o", module, "--packages", os.path.join(SDK, ".dart_tool/package_config.json")] + forge_experiment_flags(test), SDK, 300)
    if c != 0 or not os.path.exists(module):
        return {"code": -1000, "out": "", "err": ("[compile] %d " % c + o + e)[-2000:]}
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
    exe = os.path.join(d, "main")
    c, o, e = run([FORGE, "aot", test, exe, "--optimize", "--packages", os.path.join(SDK, ".dart_tool/package_config.json")] + forge_experiment_flags(test), SDK, 300)
    if c != 0 or not os.path.exists(exe):
        return {"code": -1000, "out": "", "err": ("[compile] %d " % c + o + e)[-2000:]}
    c, o, e = run([exe], d)
    return {"code": c, "out": o, "err": e[-2000:]}


def dart2wasm(test):
    d = work_dir("dart2wasm", test)
    module = os.path.join(d, "main.wasm")
    c, o, e = run([DART, "compile", "wasm"] + experiment_flags(test) + [test, "-o", module], SDK, 300)
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
    """`vm`, `dart2wasm` (only the web reference), `rejected` (a program the
    VM refuses to compile, 254, that the tool refuses too: `forge`, `wasm`,
    `native` and `dartaot`), or None (a failure)."""
    if test in ref and same(r, ref[test]):
        return "vm"
    if tool in ("forge", "wasm", "native", "dartaot") and test in ref and ref[test]["code"] == 254 and r["code"] == -1000:
        return "rejected"
    # `jit` compiles and runs in one process: a refusal exits like a compile
    # error of `dartforge` (1) with no output, after the VM refused too.
    if tool == "jit" and test in ref and ref[test]["code"] == 254 and r["code"] == 1 and not r["out"]:
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
                   "--packages=" + os.path.join(SDK, ".dart_tool/package_config.json")]
                  + experiment_flags(test) + ["-o", js, test], SDK, 120)
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
    """`equal` (the VM, or dart2wasm for `wasm`), `rejected` (by both),
    `failure`, or None when `results` lacks the program."""
    if test not in results:
        return None
    m = matches(tool, test, results[test], ref, alt)
    if m is None:
        return "failure"
    return "rejected" if m == "rejected" else "equal"


def compare(old, new, tool="forge", ref_name="vm", alt_name="dart2wasm"):
    """Each program present in both result files, by its kind in each: a
    regression goes from equal or rejected to a failure, a fix the other way;
    a move between equal and rejected by both (the VM's reference or a
    refusal changed) is listed apart."""
    vm = json.load(open(os.path.join(RES, ref_name + ".json")))
    a = json.load(open(os.path.join(RES, old + ".json")))
    b = json.load(open(os.path.join(RES, new + ".json")))
    alt_path = os.path.join(RES, alt_name + ".json")
    alt = json.load(open(alt_path)) if tool == "wasm" and os.path.exists(alt_path) else {}
    moves = {}
    for t in vm:
        ka, kb = kind(tool, t, a, vm, alt), kind(tool, t, b, vm, alt)
        if ka is None or kb is None or ka == kb:
            continue
        moves.setdefault((ka, kb), []).append(t)
    print("regressions:", sorted(moves.get(("equal", "failure"), []) + moves.get(("rejected", "failure"), [])))
    print("fixed:", len(moves.get(("failure", "equal"), [])) + len(moves.get(("failure", "rejected"), [])),
          "(%d now equal, %d now rejected by both)" % (
              len(moves.get(("failure", "equal"), [])), len(moves.get(("failure", "rejected"), []))))
    for (ka, kb), ts in sorted(moves.items()):
        if "failure" not in (ka, kb):
            print("%s -> %s: %s" % (ka, kb, sorted(ts)))

def main():
    args = sys.argv[1:]
    tool = args.pop(0)
    if tool == "setup":
        return setup(args)
    ref_name, alt_name = "vm", "dart2wasm"
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
        if args[0] in ("--forge", "--native", "--wasm", "--dartaot", "--jit"):
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
        else:
            filters.append(a)
    tests = [l.strip() for l in open(lst) if l.strip()]
    if filters:
        tests = [t for t in tests if any(f in t for f in filters)]
    fn = {"vm": vm, "dartaot": dartaot, "ddc": ddc, "forge": forge, "wasm": wasm, "native": native,
          "dart2wasm": dart2wasm, "jit": jit}[tool]
    os.makedirs(RES, exist_ok=True)
    path = os.path.join(RES, out + ".json")
    results = json.load(open(path)) if os.path.exists(path) and (filters or listed) else {}
    ref = json.load(open(os.path.join(RES, ref_name + ".json"))) if tool != "vm" else {}
    alt_path = os.path.join(RES, alt_name + ".json")
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
        count = lambda k: len([t for t in ok if kinds[t] == k])
        print("%s: %d/%d equal to the VM" % (tool, count("vm"), len(tests)))
        if tool == "wasm":
            print("%s: %d only equal to dart2wasm" % (tool, count("dart2wasm")))
        if tool in ("forge", "wasm", "native", "dartaot", "jit"):
            print("%s: %d rejected by both (the VM refuses them too)" % (tool, count("rejected")))
        print("%s: %d failures" % (tool, len(bad)))
        with open(os.path.join(RES, out + ".fail.txt"), "w") as f:
            for t in bad:
                f.write(t + "\n")
        if filters:
            for t in bad:
                print("FAIL", t, results[t]["code"], ref.get(t, {}).get("code"), (results[t]["err"] or "").strip().splitlines()[:1])

if __name__ == "__main__":
    main()
