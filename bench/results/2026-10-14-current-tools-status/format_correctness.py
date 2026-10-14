from pathlib import Path
import subprocess,os,json,hashlib,gzip
root=Path.cwd();out=root/'bench/results/2026-10-14-current-tools-status/formatter';out.mkdir(parents=True,exist_ok=True)
native=root/'target/current-toolchain-status/dartforge-native';dart=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart');sdk=dart.parent.parent/'lib'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
tracked=subprocess.check_output(['git','ls-files','*.dart'],cwd=root,text=True).splitlines()
inputs={str(root/p):sha(root/p) for p in tracked if (root/p).is_file()}
sources={'repository':[Path(p) for p in inputs], 'sdk_library':sorted(sdk.rglob('*.dart'))}
ledger={'compiler':str(native),'compiler_sha256':sha(native),'sdk':str(dart),'sdk_sha256':sha(dart),'scope':'Every tracked repository Dart file (including diagnostic corpus), plus SDK3.6.2 lib. Fresh output oracle, no writes to inputs. Rejected syntax separately counted. Exact formatted UTF8 bytes compared for jointly accepted files.','timing_measured':False,'inputs':inputs,'runs':[],'groups':{},'complete':False}
def save():(out/'validation.json').write_text(json.dumps(ledger,indent=2)+'\n')
for group,files in sources.items():
    ledger['groups'][group]={'input_files':len(files)};both={}
    for variant,tool in [('native',native),('sdk',dart)]:
        results={}
        for start in range(0,len(files),256):
            batch=files[start:start+256];label=f'{group}-{variant}-{start//256:03}'
            command=[str(tool),'format','-o','json','--summary=none','--show=none',*map(str,batch)]
            env=dict(os.environ,TMPDIR=str(root/'target/tmp-p8-runtime'))
            result=subprocess.run(command,cwd=root,env=env,capture_output=True,timeout=300)
            for stream,data in [('stdout',result.stdout),('stderr',result.stderr)]:
                with gzip.open(out/(label+'.'+stream+'.gz'),'wb') as f:f.write(data)
            row={'label':label,'command':command,'exit':result.returncode,'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest()}
            ledger['runs'].append(row)
            for line in result.stdout.decode().splitlines():
                try:record=json.loads(line)
                except json.JSONDecodeError:raise RuntimeError(f'Non-JSON formatter output {label}: {line[:100]}')
                if 'source' not in record:raise RuntimeError(f'Missing formatted source {label}')
                path=str(Path(record['path']).resolve());results[path]=record['source'].encode()
            save()
        both[variant]=results;print(group,variant,'accepted',len(results),flush=True)
    native_results,sdk_results=both['native'],both['sdk'];shared=set(native_results)&set(sdk_results)
    mismatches=[p for p in sorted(shared) if native_results[p]!=sdk_results[p]]
    ledger['groups'][group].update(native_accepted=len(native_results),sdk_accepted=len(sdk_results),jointly_accepted=len(shared),exact_bytes=len(shared)-len(mismatches),byte_mismatches=mismatches,native_only_accepted=sorted(set(native_results)-set(sdk_results)),sdk_only_accepted=sorted(set(sdk_results)-set(native_results)))
    for index,path in enumerate(mismatches[:30]):
        (out/f'{group}-mismatch-{index:03}.native.dart').write_bytes(native_results[path]);(out/f'{group}-mismatch-{index:03}.sdk.dart').write_bytes(sdk_results[path])
    save();print(group,'exact',len(shared)-len(mismatches),'/',len(shared),'mismatches',len(mismatches),flush=True)
ledger['inputs_unchanged']=all(sha(Path(p))==digest for p,digest in inputs.items());ledger['compiler_unchanged']=sha(native)==ledger['compiler_sha256'];ledger['complete']=True;save()
assert ledger['inputs_unchanged'] and ledger['compiler_unchanged']
