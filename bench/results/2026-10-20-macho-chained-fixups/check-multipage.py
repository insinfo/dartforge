from pathlib import Path
import json,subprocess,hashlib
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-chained-fixups');inspector=root/'target/string-length-validation/debug/examples/inspect_linked_images'
command=[str(inspector),str(out/'multi-arm64'),str(out/'multi-x86_64')]
result=subprocess.run(command,capture_output=True,text=True)
(out/'multipage-inspect.stdout').write_text(result.stdout);(out/'multipage-inspect.stderr').write_text(result.stderr)
assert result.returncode==0,result.stderr
images=[json.loads(line) for line in result.stdout.splitlines()];assert len(images)==2
for image in images:
 segments=image['macho_relocations']['chained']['segments']
 assert sum(s['rebases'] for s in segments)==2101,image
 assert sum(s['binds'] for s in segments)==1,image
 assert any(s['page_count']>1 and s['chains']>1 for s in segments),image
 assert all(s['overlapping_writes']==0 for s in segments),image
(out/'multipage-validation.json').write_text(json.dumps(dict(command=command,exit=result.returncode,inspector_sha256=hashlib.sha256(inspector.read_bytes()).hexdigest(),images=images,executed=False),indent=2)+'\n')
print([(image['architecture'],[(s['page_count'],s['chains'],s['rebases'],s['binds']) for s in image['macho_relocations']['chained']['segments']]) for image in images])
