"""Apply and archive controlled body edits only in an isolated explorer copy.

Window identity and preserved state must be verified separately from screenshots.
"""
from pathlib import Path
import argparse,hashlib,json,time
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('launch',type=Path)
parser.add_argument('phase',choices=['body','invalid','restore'])
parser.add_argument('--pid',required=True,type=int)
parser.add_argument('--host-pid',required=True,type=int)
parser.add_argument('--window-id',required=True,type=int)
args=parser.parse_args()
r=json.loads(args.launch.read_text());qa=args.launch.parent/'qa';qa.mkdir(exist_ok=True)
path=Path(r['body_edit_file'])
assert path.is_relative_to(Path(r['example_copy']))
sha=lambda data:hashlib.sha256(data).hexdigest()
assert all(sha(Path(p).read_bytes())==digest for p,digest in r['original_inputs'].items())
before=path.read_bytes()
if args.phase=='body':
 assert sha(before)==r['body_original_sha256']
 text=before.decode();assert text.count(r['body_edit']['old'])==1
 after=text.replace(r['body_edit']['old'],r['body_edit']['new']).encode()
elif args.phase=='invalid':
 assert before==(qa/'body-source.txt').read_bytes()
 after=before+b'\nvoid invalidJitEdit( {\n'
else:
 assert before==(qa/'invalid-source.txt').read_bytes()
 after=(qa/'body-source.txt').read_bytes()
path.write_bytes(after)
(qa/(args.phase+'-source.txt')).write_bytes(after)
record={'phase':args.phase,'file':str(path),'before_sha256':sha(before),'after_sha256':sha(after),'program_pid':args.pid,'host_pid':args.host_pid,'window_id':args.window_id,'recorded_at_epoch':time.time(),'scope':'Source mutation only; actual publication/state/window results require separate logs and observations.'}
(qa/(args.phase+'-edit.json')).write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record))
