import hashlib,json,os,pathlib,subprocess,datetime
root=pathlib.Path.cwd(); out=root/'bench/results/2026-10-14-jit-private-field-identity';out.mkdir(parents=True,exist_ok=True)
binary=root/'target/jit-readiness-build/debug/deps/dartforge_jit-c962d1984d821d80'
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
paths=[p for crate in ['jit','emit_native','runtime','types','elements','frontend'] for p in (root/'crates'/crate/'src').rglob('*.rs')]
paths += [root/'crates/runtime/effects.tsv',root/'crates/runtime/ownership.tsv']
inputs={str(p.relative_to(root)):h(p) for p in sorted(paths)}
ledger={'complete':False,'binary':str(binary),'binary_sha256':h(binary),'sources_sha256':inputs,'scope':'Actual emitted source-SDK JIT instance migration: same-spelled private fields in distinct libraries retain 101/202 and top-level turn state while a nullable field is added. ARC normal only, no broad JIT certification or timing claim.','runs':[]}
base=os.environ.copy()
for key in ['DARTFORGE_MEMORY','DARTFORGE_GC_STRESS','DARTFORGE_ARC_PREPARED','DARTFORGE_ARC_PREPARE','DARTFORGE_SDK_DLL']:base.pop(key,None)
base['DARTFORGE_MEMORY']='arc';base['DARTFORGE_ARC_CHECK']='1';base['DARTFORGE_EFFECTS']='check'
def save(): (out/'validation.json').write_text(json.dumps(ledger,indent=2)+'\n')
save()
for mode in ['legacy-arc','prepared-arc']:
 env=base.copy()
 if mode=='prepared-arc':env['DARTFORGE_ARC_PREPARED']='1'
 command=[str(binary),'emitted_private_fields_migrate_by_declaration','--include-ignored','--test-threads=1','--nocapture']
 started=datetime.datetime.now(datetime.timezone.utc).isoformat()
 result=subprocess.run(command,env=env,capture_output=True,timeout=1500)
 (out/(mode+'.stdout')).write_bytes(result.stdout);(out/(mode+'.stderr')).write_bytes(result.stderr)
 row={'mode':mode,'command':command,'environment':{k:v for k,v in env.items() if k.startswith('DARTFORGE_') or k in ['TMPDIR','CARGO_TARGET_DIR','SDKROOT','LLVM_SYS_221_PREFIX','RUSTFLAGS']},'started_utc':started,'ended_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'exit_code':result.returncode,'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest(),'passed':result.returncode==0 and b'1 passed; 0 failed' in result.stdout and b' ... 33\n303\nok\n' in result.stdout}
 ledger['runs'].append(row);save();print(mode,row['exit_code'],row['passed'],flush=True)
 assert h(binary)==ledger['binary_sha256'];assert all(h(root/p)==value for p,value in inputs.items())
ledger['complete']=True;ledger['inputs_stable']=True;save()
