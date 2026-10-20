set -euo pipefail
snapshot=$(cat /tmp/forge-current-desktop-linux-snapshot)
python3 - "$snapshot" <<'PYCODE'
from pathlib import Path
import tarfile,hashlib,json,sys
shared=Path('/media/psf/ssd_dock/MyRustProjects/dartforge/target/current-desktop-validation');archive=shared/'source-fixed-snapshot.tar.gz'
assert hashlib.sha256(archive.read_bytes()).hexdigest()=='c33aa08c8f0654ae8c562c3fb3662389fd22a7640841db88f581fea3cac10e8b'
with tarfile.open(archive) as source:source.extractall(sys.argv[1],filter='data')
rows=json.loads((shared/'fixed-source-files.json').read_text())
for row in rows:assert hashlib.sha256((Path(sys.argv[1])/row['path']).read_bytes()).hexdigest()==row['sha256'],row['path']
report=dict(snapshot=sys.argv[1],files=len(rows),archive_sha256=hashlib.sha256(archive.read_bytes()).hexdigest())
(shared/'linux-source-verification-fixed.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
PYCODE
chown -R isaque:isaque "$snapshot"
