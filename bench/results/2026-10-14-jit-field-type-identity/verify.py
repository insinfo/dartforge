import hashlib,json,os,pathlib,subprocess,datetime
root=pathlib.Path.cwd();out=root/'bench/results/2026-10-14-jit-field-type-identity';binary=root/'target/jit-readiness-build/debug/deps/dartforge_jit-c962d1984d821d80'
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
paths=[p for crate in ['jit','emit_native','runtime','types','elements','frontend'] for p in (root/'crates'/crate/'src').rglob('*.rs')]+[root/'crates/runtime/effects.tsv',root/'crates/runtime/ownership.tsv']
current_inputs={str(p.relative_to(root)):h(p) for p in sorted(paths)}
previous=json.loads((out/'validation.json').read_text()) if (out/'validation.json').exists() else None
inputs=previous['source_sha256'] if previous else current_inputs
ledger={'complete':False,'binary':str(binary),'binary_sha256':h(binary),'source_sha256':inputs,'runs':[],'scope':'Actual source-SDK JitSession rejects a.Foo→b.Foo before publication, original object mutation101 and turn counter survive rejection, Alias=a.Foo then publishes and preserves state. Three memory modes, normal and GC stress; no timing or general hot-reload/type-system certification.','expected_observations':[102,103,104]}
if previous:
 ledger=previous
 ledger['workspace_delta_policy']='Only unrelated runtime/src/natives_strings.rs source edits allowed after immutable testbinary build; compiled-source snapshot remains original. This is not a gate for the edited runtime. Parent authorized continuing already-passed cases without recompilation.'
 ledger['complete']=False
 assert h(binary)==ledger['binary_sha256']
base=os.environ.copy()
for key in ['DARTFORGE_MEMORY','DARTFORGE_ARC_PREPARED','DARTFORGE_ARC_PREPARE','DARTFORGE_GC_STRESS','DARTFORGE_SDK_DLL']:base.pop(key,None)
base['DARTFORGE_ARC_CHECK']='1';base['DARTFORGE_EFFECTS']='check'
def save():(out/'validation.json').write_text(json.dumps(ledger,indent=2)+'\n')
save()
for mode in ['tracing','legacy-arc','prepared-arc']:
 for stress in [False,True]:
  env=base.copy();env['DARTFORGE_MEMORY']='tracing' if mode=='tracing' else 'arc'
  if mode=='prepared-arc':env['DARTFORGE_ARC_PREPARED']='1'
  if stress:env['DARTFORGE_GC_STRESS']='1'
  label=mode+('-stress' if stress else '-normal')
  if any(row['label']==label and row['passed'] for row in ledger['runs']):
   print(label+' previously passed; preserved',flush=True);continue
  command=[str(binary),'emitted_same_spelling_field_type_change_rejects_before_publication','--include-ignored','--nocapture','--test-threads=1']
  started=datetime.datetime.now(datetime.timezone.utc).isoformat();result=subprocess.run(command,env=env,capture_output=True,timeout=1500)
  (out/(label+'.stdout')).write_bytes(result.stdout);(out/(label+'.stderr')).write_bytes(result.stderr)
  passed=result.returncode==0 and b'1 passed; 0 failed' in result.stdout and b' ... 102\n103\n104\nok\n' in result.stdout
  ledger['runs'].append({'label':label,'mode':mode,'gc_stress':stress,'command':command,'environment':{k:v for k,v in env.items() if k.startswith('DARTFORGE_') or k in ['TMPDIR','SDKROOT','CARGO_TARGET_DIR','RUSTFLAGS','LLVM_SYS_221_PREFIX']},'started_utc':started,'ended_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'exit_code':result.returncode,'passed':passed,'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest()});save();print(label,result.returncode,passed,flush=True)
  assert passed,label;assert h(binary)==ledger['binary_sha256'];deltas={p:{'compiled_snapshot':value,'workspace_current':h(root/p)} for p,value in inputs.items() if h(root/p)!=value}
  ledger['workspace_source_changes']=deltas;save()
  assert set(deltas).issubset({'crates/runtime/src/natives_strings.rs'}),'backend/JIT or other source changed'
ledger['complete']=True;ledger['binary_and_backend_jit_inputs_stable']=True;ledger['all_workspace_sources_unchanged']=not ledger.get('workspace_source_changes');save()
