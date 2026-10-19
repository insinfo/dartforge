from pathlib import Path
import subprocess
out = Path('bench/results/2026-10-19-build-quality')
source = Path('crates/build/src/packages.rs')
old = subprocess.check_output(['git', 'show', 'ee6e289a:crates/build/src/packages.rs'], text=True)
new = source.read_text()
def function(text):
    return text[text.index('pub fn scc('):text.index('#[cfg(test)]')]
code = 'use std::{hint::black_box, time::Instant};\n'
for label, text in [('before', old), ('after', new)]:
    code += 'mod '+label+' { use std::collections::HashMap;\n'+function(text)+'}\n'
code += '''
fn main() {
    println!("shape,nodes,sample,before_ns,after_ns");
    for shape in ["star", "chain", "cycle"] {
        for count in [1000, 5000, 10000] {
            let graph: Vec<Vec<usize>> = (0..count).map(|n| match shape {
                "star" if n == 0 => (1..count).collect(),
                "chain" if n + 1 < count => vec![n+1],
                "cycle" => vec![(n+1)%count],
                _ => vec![],
            }).collect();
            assert_eq!(before::scc(&[0], |n| graph[n].clone()), after::scc(&[0], |n| graph[n].clone()));
            for sample in 0..9 {
                let mut elapsed = [0, 0];
                for mode in if sample % 2 == 0 { [0, 1] } else { [1, 0] } {
                    let start = Instant::now();
                    let result = if mode == 0 {
                        before::scc(black_box(&[0]), |n| graph[n].clone())
                    } else {
                        after::scc(black_box(&[0]), |n| graph[n].clone())
                    };
                    black_box(result);
                    elapsed[mode] = start.elapsed().as_nanos();
                }
                println!("{shape},{count},{sample},{},{}", elapsed[0], elapsed[1]);
            }
        }
    }
}
'''
Path('/tmp/forge-scc-benchmark.rs').write_text(code)
subprocess.run(['rustc','--edition=2024','-O','/tmp/forge-scc-benchmark.rs','-o','/tmp/forge-scc-benchmark'],check=True)
with (out/'scc-benchmark.csv').open('w') as log:
    subprocess.run(['/tmp/forge-scc-benchmark'],stdout=log,check=True)
