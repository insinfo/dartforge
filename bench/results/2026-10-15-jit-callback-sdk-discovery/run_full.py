import hashlib,json,pathlib,re,subprocess,os
out=pathlib.Path(__file__).parent
log=(out/'tests.log').read_text()
assert 'test result: ok. 2 passed; 0 failed;' in log
binary=re.search(r'Running unittests [^\n]+ \(([^)]+)\)',log).group(1)
assert 'DARTFORGE_TEST_SDK_LIB' not in os.environ
command=[binary,'--include-ignored','--nocapture']
sha=hashlib.sha256(pathlib.Path(binary).read_bytes()).hexdigest()
with (out/'full.log').open('w') as stream:
 result=subprocess.run(command,stdout=stream,stderr=subprocess.STDOUT)
source_before=json.loads((out/'source-before.json').read_text())
source_after={p:hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest() for p in source_before}
(out/'result.json').write_text(json.dumps({'command':command,'binary_sha256':sha,'exit':result.returncode,'test_sdk_override_unset':True,'selected_sdk':os.environ['DARTFORGE_SDK_LIB'],'source_unchanged':source_before==source_after,'source_after':source_after},indent=2)+'\n')
raise SystemExit(result.returncode)
