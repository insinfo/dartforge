import pathlib,json,os,subprocess,hashlib,time
root=pathlib.Path(__file__).resolve().parents[2];base=root/'target/test-runner-final';cli=root/'target/current-toolchain-status/dartforge-native';sdk=pathlib.Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
inputs=[p for tree in ['project','configured-project','controls'] for p in (base/tree).rglob('*') if p.is_file()]+[root/'crates/cli/src/test_runner.rs',root/'crates/cli/src/test_runner/worker.dart']
initial={str(p):sha(p) for p in inputs};rows=[]
def run(label,args,expected,mode='',cwd=None):
 env=os.environ.copy();env['TEST_FAILURE']=mode
 command=[str(cli),'test',*map(str,args),'--sdk',str(sdk),'--keep-artifacts']
 t=time.time();p=subprocess.run(command,cwd=cwd or base/'project',env=env,capture_output=True,timeout=240)
 for suffix,data in [('stdout',p.stdout),('stderr',p.stderr)]: (base/'raw'/(label+'.'+suffix)).write_bytes(data)
 row={'label':label,'command':command,'cwd':str(cwd or base/'project'),'TEST_FAILURE':mode,'exit_code':p.returncode,'expected_exit':expected,'passed':p.returncode==expected,'started_unix':t,'stdout_sha256':sha(base/'raw'/(label+'.stdout')),'stderr_sha256':sha(base/'raw'/(label+'.stderr'))};rows.append(row);print(label,p.returncode,expected,flush=True)
 if p.returncode!=expected: print(p.stdout.decode(),p.stderr.decode(),flush=True)
run('discovery-pass',['--reporter','json'],0)
run('assertion-failure',['--reporter','json'],1,'assertion')
run('unawaited-async-failure',['--reporter','json'],1,'async')
run('name-regex',['--name',r'inner (async|callback)$','--reporter','json'],0)
run('plain-name',['--plain-name','callback','--reporter','json'],0)
run('no-match',['--name','not-an-existing-test','--reporter','json'],79)
cfg=base/'project/.dart_tool/package_config.json'
run('early-exit-zero',[base/'controls/early_exit_test.dart','--packages',cfg],1)
run('annotations-refused',[base/'controls/annotated_test.dart','--packages',cfg],1)
run('yaml-refused',[],1,cwd=base/'configured-project')
run('platform-refused',['--platform','chrome'],1)
run('missing-main',[base/'controls/no_main_test.dart','--packages',cfg],1)
record={'cli':str(cli),'cli_sha256':sha(cli),'sdk':str(sdk),'source_and_fixture_sha256':initial,'inputs_stable':all(sha(pathlib.Path(p))==h for p,h in initial.items()),'cases':rows,'all_expected_exits':all(row['passed'] for row in rows),'limitations':['Correctness protocol executions, no speed benchmark.','Actual dartforge test command compiles and runs native workers; no SDK test runner delegation.']};(base/'validation.json').write_text(json.dumps(record,indent=2)+'\n')
if not record['all_expected_exits'] or not record['inputs_stable']:raise SystemExit(1)
