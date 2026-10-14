"""Archive manifests and raw logs, never compiler binaries or generated bundles."""
from pathlib import Path
import shutil
ROOT=Path.cwd();SOURCE=ROOT/'target/current-js-status';DEST=ROOT/'bench/results/2026-10-14-current-js-status'
for path in SOURCE.rglob('*'):
    if not path.is_file():continue
    relative=path.relative_to(SOURCE)
    if path.suffix not in ['.json','.log','.stdout','.stderr']:continue
    if 'correctness' in relative.parts and path.suffix=='.dart':continue
    if path.name=='package.json':continue
    destination=DEST/relative;destination.parent.mkdir(parents=True,exist_ok=True)
    shutil.copy2(path,destination)
print('Evidence archived; ignored log/stdout/stderr files require force-add.')
