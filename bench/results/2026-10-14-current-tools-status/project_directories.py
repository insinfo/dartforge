from pathlib import Path
import os,subprocess,json,hashlib
root=Path.cwd();out=root/'bench/results/2026-10-14-current-tools-status/project-directories';out.mkdir(exist_ok=True)
native=root/'target/current-toolchain-status/dartforge-native';dart=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart')
projects={'file_explorer':Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer'),'ordinary_pub_project':root/'target/native-provenance/native-pub-get-validation/hosted-native'}
ledger={'compiler_sha256':hashlib.sha256(native.read_bytes()).hexdigest(),'sdk_sha256':hashlib.sha256(dart.read_bytes()).hexdigest(),'timing_measured':False,'projects':[]}
for label,directory in projects.items():
 reports={};row={'project':label,'directory':str(directory),'input_hashes':{str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in directory.rglob('*.dart') if '.dart_tool' not in p.parts},'runs':[]}
 for variant,command in [('sdk',[dart,'analyze','--format=json',directory]),('native',[native,'analyze','--format=json',directory]),('native-all',[native,'analyze','--format=json','--all',directory])]:
  result=subprocess.run(list(map(str,command)),cwd=directory,env=dict(os.environ,TMPDIR=str(root/'target/tmp-p8-runtime'),DARTFORGE_SDK_LIB=str(dart.parent.parent/'lib')),capture_output=True,timeout=600)
  (out/f'{label}-{variant}.stdout').write_bytes(result.stdout);(out/f'{label}-{variant}.stderr').write_bytes(result.stderr)
  reports[variant]=json.loads(result.stdout);row['runs'].append({'variant':variant,'command':list(map(str,command)),'exit':result.returncode,'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest(),'diagnostics':len(reports[variant]['diagnostics'])})
 row['default_exact']=reports['native']==reports['sdk'];row['all_exact']=reports['native-all']==reports['sdk'];row['missing_default']=[x for x in reports['sdk']['diagnostics'] if x not in reports['native']['diagnostics']];row['additional_default']=[x for x in reports['native']['diagnostics'] if x not in reports['sdk']['diagnostics']];row['missing_all']=[x for x in reports['sdk']['diagnostics'] if x not in reports['native-all']['diagnostics']];row['additional_all']=[x for x in reports['native-all']['diagnostics'] if x not in reports['sdk']['diagnostics']]
 ledger['projects'].append(row);(out/'validation.json').write_text(json.dumps(ledger,indent=2)+'\n');print(label,'default',row['default_exact'],'all',row['all_exact'],flush=True)
ledger['complete']=True;(out/'validation.json').write_text(json.dumps(ledger,indent=2)+'\n')
