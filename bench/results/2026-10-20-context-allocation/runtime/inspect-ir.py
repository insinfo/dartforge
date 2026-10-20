from pathlib import Path
import json,hashlib
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');p=root/'target/context-allocation-validation/.df_tmp';rows=[]
for name in ['production','development','unpruned','linux-arm64-fixture','windows-x64-fixture']:
 a=(p/f'before-{name}.ll').read_bytes();b=(p/f'after-{name}.ll').read_bytes();rows.append(dict(name=name,before_bytes=len(a),after_bytes=len(b),identical=a==b,before_sha256=hashlib.sha256(a).hexdigest(),after_sha256=hashlib.sha256(b).hexdigest()));assert a==b,name
Path('/tmp/forge-context-allocation/runtime/ir-comparison.json').write_text(json.dumps(rows,indent=2)+'\n');print('Five full assembled IR comparisons are byte-identical.')
