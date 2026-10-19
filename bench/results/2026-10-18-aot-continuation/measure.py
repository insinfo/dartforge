"""Run paired final binaries after compilation and correctness tests have finished."""
import json
import pathlib
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[3]
OUT = pathlib.Path(__file__).resolve().parent
variants = {'a0': ROOT/'target/bench-continuation-a0', 'a1': ROOT/'target/bench-continuation-a1', 'dart': ROOT/'target/bench-continuation-dart'}
records = []
checksums = {}
for iteration in range(5):
    order = list(variants)
    order = order[iteration % 3:] + order[:iteration % 3]
    for name in order:
        result = subprocess.run([str(variants[name])], capture_output=True, text=True, check=True, timeout=60)
        (OUT/f'bench.{name}.{iteration}.out').write_text(result.stdout)
        for line in result.stdout.splitlines():
            match = re.fullmatch(r'(\w+): ([\d ]+) ms \| (-?\d+)', line)
            assert match, line
            kernel, times, checksum = match.groups()
            assert checksums.setdefault(kernel, checksum) == checksum, (kernel, checksum)
            records.append(dict(iteration=iteration, variant=name, kernel=kernel, milliseconds=list(map(int,times.split())), checksum=checksum))
colors=[]
color_checksum=None
for iteration in range(5):
    for name in (['a0','dart'] if iteration % 2 == 0 else ['dart','a0']):
        executable=ROOT/f'target/color-continuation-{name}'
        result=subprocess.run([str(executable)],capture_output=True,text=True,check=True,timeout=60)
        (OUT/f'color.{name}.{iteration}.out').write_text(result.stdout)
        for line in result.stdout.splitlines():
            checksum,time=line.split(':')
            if color_checksum is None: color_checksum=checksum
            assert checksum==color_checksum
            colors.append(dict(iteration=iteration,variant=name,microseconds=int(time),checksum=checksum))
summary={kernel:{name:min(t for r in records if r['variant']==name and r['kernel']==kernel for t in r['milliseconds']) for name in variants} for kernel in checksums}
summary['color_us']={name:min(r['microseconds'] for r in colors if r['variant']==name) for name in ['a0','dart']}
summary['bytes']={name:path.stat().st_size for name,path in variants.items()}
(OUT/'performance.json').write_text(json.dumps(dict(rounds=5,bench=records,color=colors,summary=summary),indent=2)+'\n')
print(json.dumps(summary,indent=2))
