"""Freeze the standalone JS compiler's actual workspace dependency closure."""
from pathlib import Path
import hashlib,json,os,subprocess,shutil
root=Path(__file__).resolve().parents[3]
out=root/'bench/results/2026-10-15-js-fresh-list/producer-v2'
out.mkdir(exist_ok=True)
env=os.environ.copy()
env.update(CARGO_BUILD_JOBS='2',CARGO_TARGET_DIR=str(root/'target/p4'),TMPDIR='/Volumes/ssd_dock/tmp/dartforge-js-fresh-list',DARTFORGE_SDK_LIB='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib')
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
metadata=json.loads(subprocess.check_output(['cargo','metadata','--locked','--format-version','1'],env=env,text=True))
packages={p['id']:p for p in metadata['packages']};nodes={n['id']:n for n in metadata['resolve']['nodes']};start=next(p['id'] for p in packages.values() if p['name']=='dartforge-emit-js-production' and p['source'] is None)
seen=set();pending=[start]
while pending:
 key=pending.pop()
 if key in seen:continue
 seen.add(key);pending.extend(nodes[key]['dependencies'])
workspace=[packages[k] for k in seen if packages[k]['source'] is None]
files={root/'Cargo.toml',root/'Cargo.lock'}
config=root/'.cargo/config.toml'
if config.exists():files.add(config)
for package in workspace:
 folder=Path(package['manifest_path']).parent
 files.add(Path(package['manifest_path']))
 for directory in ['src']:
  files.update(p for p in (folder/directory).rglob('*') if p.is_file())
 build=folder/'build.rs'
 if build.exists():files.add(build)
snapshot={str(p.relative_to(root)):sha(p) for p in sorted(files)}
(out/'source-sha256.json').write_text(json.dumps(snapshot,indent=2)+'\n')
provenance={'git_head_start':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'dependency_packages':sorted(p['name'] for p in workspace),'source_scope':'Root manifests/config and complete src trees/build.rs for workspace packages transitively reachable from dartforge-emit-js-production in locked cargo metadata; unrelated CLI/JIT/native sources excluded.','rustc':subprocess.check_output(['rustc','--version','--verbose'],text=True),'cargo':subprocess.check_output(['cargo','--version'],text=True),'env':{k:env.get(k) for k in ['CARGO_BUILD_JOBS','CARGO_TARGET_DIR','TMPDIR','DARTFORGE_SDK_LIB','RUSTFLAGS','CARGO_HOME','RUSTUP_HOME']}}
(out/'provenance.json').write_text(json.dumps(provenance,indent=2)+'\n')
command=['cargo','build','--locked','--release','-p','dartforge-emit-js-production','--bin','dartforge-jsprod']
with (out/'build.log').open('wb') as log:result=subprocess.run(command,env=env,stdout=log,stderr=subprocess.STDOUT)
changed=[p for p,h in snapshot.items() if not (root/p).is_file() or sha(root/p)!=h]
report={'command':command,'exit_code':result.returncode,'sources_unchanged':not changed,'changed_inputs':changed,'git_head_end':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()}
if result.returncode==0 and not changed:
 folder=root/'target/js-fresh-list-producer-v2';folder.mkdir(exist_ok=True);binary=folder/'dartforge-jsprod';shutil.copy2(root/'target/p4/release/dartforge-jsprod',binary)
 report.update(binary=str(binary),binary_sha256=sha(binary),binary_bytes=binary.stat().st_size)
(out/'result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
assert result.returncode==0 and not changed
