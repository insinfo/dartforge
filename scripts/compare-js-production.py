#!/usr/bin/env python3
"""Speed and size of the production JavaScript against `dart2js`.

    scripts/compare-js-production.py compile TAG [--jsprod BIN] [--level -O4] [programs...]
    scripts/compare-js-production.py dart2js [--level -O4] [programs...]   (tag dart2js-O4)
    scripts/compare-js-production.py run TAG TAG... [--reps N] [programs...]
    scripts/compare-js-production.py size TAG TAG... [programs...]

The programs are `bench/performance/*.dart` and a few of `bench/comparison`
(`docs/JS-PRODUCAO-VELOCIDADE.md` §1 and §11). `collections.dart` uses 64-bit
integers that dart2js rejects: the work copy replaces them by 2^53 - 1 and
2^30, as the earlier rounds did. Each program's `Stopwatch` runs with
`performance.now` replaced by the CPU time of Node's main thread
(`process.threadCpuUsage`), which varies much less than the wall clock on a
loaded machine; a kernel's time is the median of its rounds after the first,
then the median over the repetitions, which alternate the tags. The output
of every program (without the times) is compared between the tags; a
difference is reported, not hidden.

Environment: `DART` (default `dart` on PATH), `NODE` (default `node`),
`DARTFORGE_JSPROD` (default `target/release/dartforge-jsprod`), `OUT`
(default `target/js-production`).
"""
import gzip, json, os, re, shutil, statistics, subprocess, sys, time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.environ.get('OUT') or os.path.join(ROOT, 'target', 'js-production')
SRC = os.path.join(OUT, 'src')
NODE = os.environ.get('NODE') or shutil.which('node') or 'node'
DART = os.environ.get('DART') or shutil.which('dart') or 'dart'
JSPROD = os.environ.get('DARTFORGE_JSPROD') or os.path.join(ROOT, 'target', 'release', 'dartforge-jsprod')
PERFORMANCE = ['calls', 'collections', 'escaping_objects', 'fresh_list_append', 'json', 'numeric',
               'objects_in_collections', 'strings', 'temporary_objects', 'typed_data', 'typed_data_widths']
COMPARISON = ['collection_operations', 'utf8_convert', 'json_encode', 'bench']
PROGRAMS = PERFORMANCE + COMPARISON
# The program's `performance.now` is the main thread's CPU time; `window`
# makes dart2js's `Stopwatch` use `performance.now` too.
PRELUDE = ("globalThis.window = globalThis;"
           "{const c=()=>{const u=process.threadCpuUsage();return u.user/1000;};"
           "Object.defineProperty(globalThis,'performance',{value:{now:c},configurable:true,writable:true});}\n")
LINE = re.compile(r'^(\w+): ([\d ]+) (us|ms) \| (.*)$')


def sources():
    """The work copy of the programs (`common.dart` next to every one)."""
    os.makedirs(SRC, exist_ok=True)
    perf = os.path.join(ROOT, 'bench', 'performance')
    comp = os.path.join(ROOT, 'bench', 'comparison')
    shutil.copy(os.path.join(perf, 'common.dart'), SRC)
    for p in PERFORMANCE:
        text = open(os.path.join(perf, p + '.dart')).read()
        if p == 'collections':
            text = (text.replace('const mn = -9223372036854775808, mx = 9223372036854775807;',
                                 'const mn = -9007199254740991, mx = 9007199254740991;')
                    .replace('(1 << 62)', '(1 << 30)')
                    .replace('var x = 88172645463325252;', 'var x = 88172645463325;'))
        open(os.path.join(SRC, p + '.dart'), 'w').write(text)
    for p in COMPARISON:
        text = open(os.path.join(comp, p + '.dart')).read()
        open(os.path.join(SRC, p + '.dart'), 'w').write(
            text.replace("import '../performance/common.dart';", "import 'common.dart';"))


def options(argv):
    o, rest = {}, []
    i = 0
    while i < len(argv):
        if argv[i].startswith('--'):
            o[argv[i][2:]] = argv[i + 1]
            i += 2
        else:
            rest.append(argv[i])
            i += 1
    return o, rest


def compile_tag(tag, opts, programs, dart2js=False):
    sources()
    d = os.path.join(OUT, tag)
    os.makedirs(d, exist_ok=True)
    level = opts.get('level', '-O4')
    for p in programs:
        src = os.path.join(SRC, p + '.dart')
        js = os.path.join(d, p + '.js')
        t = time.time()
        cmd = ([DART, 'compile', 'js', level, '-o', js, src] if dart2js
               else [opts.get('jsprod', JSPROD), src, '-o', js, level])
        r = subprocess.run(cmd, capture_output=True, text=True)
        if r.returncode != 0:
            print(f'{tag} {p}: FAILED\n{r.stdout[-2000:]}{r.stderr[-2000:]}')
            continue
        body = open(js).read()
        open(os.path.join(d, p + '.run.js'), 'w').write(PRELUDE + body)
        print(f'{tag} {p}: {len(body)} bytes, {time.time() - t:.1f} s', flush=True)


def parse(program, out):
    res = {}
    if program == 'json_encode':
        report = json.loads(out.strip().splitlines()[-1])
        for s in report['scenarios']:
            for k in ('json', 'string_buffer'):
                m = s[k]
                res[f"{s['scenario']}_{k}"] = (m['elapsed_micros'] / m['iterations'], str(m['checksum']))
        return res
    for line in out.splitlines():
        m = LINE.match(line)
        if m:
            ts = [int(x) for x in m.group(2).split()]
            if m.group(3) == 'us':
                ts = [x / 1000 for x in ts]
            res[m.group(1)] = (statistics.median(ts[1:]) if len(ts) > 1 else ts[0], m.group(4))
    return res


def run(tags, opts, programs):
    reps = int(opts.get('reps', 5))
    times, results, outputs = {}, {}, {}
    for rep in range(reps):
        for p in programs:
            for tag in (tags if rep % 2 == 0 else list(reversed(tags))):
                js = os.path.join(OUT, tag, p + '.run.js')
                if not os.path.exists(js):
                    continue
                r = subprocess.run([NODE, js], capture_output=True, text=True, timeout=900)
                if r.returncode != 0:
                    print(f'{tag} {p}: exit {r.returncode}\n{r.stderr[-1500:]}')
                    continue
                for k, (v, chk) in parse(p, r.stdout).items():
                    times.setdefault((p, k), {}).setdefault(tag, []).append(v)
                    results.setdefault((p, k), {})[tag] = chk
                if p != 'json_encode':
                    outputs.setdefault(p, {})[tag] = '\n'.join(
                        l for l in r.stdout.splitlines() if not LINE.match(l))
        print(f'repetition {rep + 1}/{reps}', file=sys.stderr, flush=True)
    print('| kernel (ms) | ' + ' | '.join(tags) + f' | {tags[0]}/{tags[-1]} |')
    print('| --- |' + ' ---: |' * (len(tags) + 1))
    for (p, k), per in times.items():
        meds = [statistics.median(per[t]) if t in per else float('nan') for t in tags]
        differs = ' (result differs)' if len(set(results[(p, k)].values())) > 1 else ''
        print(f'| `{p}/{k}`{differs} | ' + ' | '.join(f'{m:.2f}' for m in meds) + f' | {meds[0] / meds[-1]:.2f} |')
    for p, per in outputs.items():
        if len(set(per.values())) > 1:
            print(f'output differs: {p} ({", ".join(per)})')


def size(tags, programs):
    print('| program | ' + ' | '.join(f'{t} | {t} gzip -9' for t in tags) + ' |')
    print('| --- |' + ' ---: |' * (2 * len(tags)))
    total = {t: [0, 0] for t in tags}
    for p in programs:
        row = []
        for t in tags:
            js = os.path.join(OUT, t, p + '.js')
            if not os.path.exists(js):
                row += ['', '']
                continue
            b = open(js, 'rb').read()
            g = len(gzip.compress(b, 9))
            total[t][0] += len(b)
            total[t][1] += g
            row += [str(len(b)), str(g)]
        print(f'| `{p}` | ' + ' | '.join(row) + ' |')
    print('| total | ' + ' | '.join(f'{total[t][0]} | {total[t][1]}' for t in tags) + ' |')


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    cmd = sys.argv[1]
    opts, rest = options(sys.argv[2:])
    if cmd == 'compile':
        compile_tag(rest[0], opts, rest[1:] or PROGRAMS)
    elif cmd == 'dart2js':
        compile_tag('dart2js' + opts.get('level', '-O4'), opts, rest or PROGRAMS, dart2js=True)
    elif cmd in ('run', 'size'):
        tags = [t for t in rest if os.path.isdir(os.path.join(OUT, t))]
        programs = [p for p in rest if p not in tags] or PROGRAMS
        (run(tags, opts, programs) if cmd == 'run' else size(tags, programs))
    else:
        sys.exit(__doc__)


main()
