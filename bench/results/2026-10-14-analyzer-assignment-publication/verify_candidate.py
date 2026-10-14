from pathlib import Path
import os,json,subprocess,hashlib,shutil,argparse
parser=argparse.ArgumentParser();parser.add_argument('--compiler',type=Path,required=True);parser.add_argument('--label',required=True);args=parser.parse_args()
root=Path.cwd();base=root/'bench/results/2026-10-14-analyzer-assignment-publication';out=base/args.label;out.mkdir(parents=True,exist_ok=True);work=root/'target/analyzer-assignment-publication';work.mkdir(exist_ok=True)
native=args.compiler.resolve();dart=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart');env=dict(os.environ,TMPDIR=str(root/'target/tmp-p8-runtime'),DARTFORGE_SDK_LIB=str(dart.parent.parent/'lib'))
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
cases={
'literal':"void main() { int value = 'wrong'; print(value); }\n",
'inferred_write':"void main() { var value = 1; value = 'wrong'; print(value); }\n",
'field_initializer':"class Box { int value = 'wrong'; } void main() { print(Box().value); }\n",
'static_initializer':"class Box { static int value = 'wrong'; } void main() { print(Box.value); }\n",
'top_level_initializer':"int value = 'wrong'; void main() { print(value); }\n",
'function_value':"String text() => 'ok'; void main() { int Function() value = text; print(value); }\n",
'num_parameter':"void consume(num input) { int value = input; print(value); }\n",
'nullable_parameter':"void consume(String? input) { String value = input; print(value); }\n",
'generic_parameter':"void consume(List<String> input) { List<int> value = input; print(value); }\n",
'valid_numeric_literal':"void main() { double value = 1; print(value); }\n",
'valid_supertype':"void main() { Object value = 'ok'; print(value); }\n",
'valid_dynamic':"void consume(dynamic input) { int value = input; print(value); }\n",
'valid_implicit_call':"class Callable { int call() => 1; } void main() { int Function() value = Callable(); print(value); }\n",
'valid_never':"Never fail() => throw 'error'; void main() { int value = fail(); print(value); }\n",
'setter':"class Box { set value(int input) {} } void main() { Box().value = 'wrong'; }\n",
}
fixture=work/'ordinary';(fixture/'lib').mkdir(parents=True,exist_ok=True);(fixture/'pubspec.yaml').write_text('name: assignment_publication\nenvironment:\n  sdk: ^3.6.0\n');(fixture/'.dart_tool').mkdir(exist_ok=True);(fixture/'.dart_tool/package_config.json').write_text(json.dumps({'configVersion':2,'packages':[{'name':'assignment_publication','rootUri':'../','packageUri':'lib/','languageVersion':'3.6'}]}))
targets=[]
for name,text in cases.items():
 p=fixture/'lib'/(name+'.dart');p.write_text(text+'// Assignment publication oracle: '+name+'\n');targets.append((name,p,'ordinary'))
targets.extend([('file_explorer',Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer'),'real_project'),('dart_ui_library',Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/lib'),'real_library')])
hosted=root/'target/native-provenance/native-pub-get-validation/hosted-native';config=json.loads((hosted/'.dart_tool/package_config.json').read_text())
from urllib.parse import urlparse
from urllib.request import url2pathname
for entry in config['packages']:
 if entry['name']=='hosted_app':continue
 source=Path(url2pathname(urlparse(entry['rootUri']).path));destination=work/'hosted-libraries'/entry['name'];destination.mkdir(parents=True,exist_ok=True)
 if not (destination/'lib').exists():shutil.copytree(source/'lib',destination/'lib')
 shutil.copy2(source/'pubspec.yaml',destination/'pubspec.yaml');(destination/'.dart_tool').mkdir(exist_ok=True);(destination/'.dart_tool/package_config.json').write_text(json.dumps(config));targets.append(('hosted-'+entry['name'],destination/'lib','real_library'))
ledger={'label':args.label,'compiler':str(native),'compiler_sha256':sha(native),'sdk_executable_sha256':sha(dart),'sdk_version':'3.6.2','timing_measured':False,'scope':'Fresh SDK3.6.2 oracles for15ordinary invalid/valid cases, FileExplorer wholeproject, dart_ui library and12current downloaded hostedpackage libraries. Only invalid_assignment is candidate; othercode differences are retained separately. No historical209packageclaim.','targets':[]}
for label,target,kind in targets:
 reports={};row={'label':label,'kind':kind,'path':str(target),'input_hashes':{str(p):sha(p) for p in ([target] if target.is_file() else target.rglob('*.dart'))},'runs':[]}
 for variant,command in [('sdk',[dart,'analyze','--format=json',target]),('native-all',[native,'analyze','--format=json','--all',target])]+([('native-default',[native,'analyze','--format=json',target])] if kind=='ordinary' or label=='file_explorer' else []):
  result=subprocess.run(list(map(str,command)),cwd=target.parent if target.is_file() else target,env=env,capture_output=True,timeout=600)
  (out/f'{label}-{variant}.stdout').write_bytes(result.stdout);(out/f'{label}-{variant}.stderr').write_bytes(result.stderr);reports[variant]=json.loads(result.stdout)
  row['runs'].append({'variant':variant,'command':list(map(str,command)),'exit':result.returncode,'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest()})
 sdk_records=[r for r in reports['sdk']['diagnostics'] if r['code']=='invalid_assignment'];actual=[r for r in reports['native-all']['diagnostics'] if r['code']=='invalid_assignment'];row.update(candidate_sdk=sdk_records,candidate_native=actual,candidate_exact=sdk_records==actual,full_all_exact=reports['sdk']==reports['native-all'],default_exact=reports.get('native-default')==reports['sdk'] if 'native-default' in reports else None)
 ledger['targets'].append(row);(out/'validation.json').write_text(json.dumps(ledger,indent=2)+'\n');print(label,'candidateexact',row['candidate_exact'],'sdkcount',len(sdk_records),'allfull',row['full_all_exact'],'default',row['default_exact'],flush=True)
ledger['candidate_exact_all']=all(r['candidate_exact'] for r in ledger['targets']);ledger['candidate_sdk_count']=sum(len(r['candidate_sdk']) for r in ledger['targets']);ledger['input_files']=sum(len(r['input_hashes']) for r in ledger['targets']);ledger['compiler_unchanged']=sha(native)==ledger['compiler_sha256'];ledger['complete']=True;(out/'validation.json').write_text(json.dumps(ledger,indent=2)+'\n');assert ledger['candidate_exact_all'] and ledger['compiler_unchanged']
