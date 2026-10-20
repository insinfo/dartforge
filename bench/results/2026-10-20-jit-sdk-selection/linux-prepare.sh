set -euo pipefail
if test -f /tmp/forge-current-desktop-linux-snapshot; then
  snapshot=$(cat /tmp/forge-current-desktop-linux-snapshot)
else
  snapshot=$(mktemp -d /tmp/dartforge-current-desktop-source.XXXXXX)
fi
python3 - "$snapshot" <<'PY'
from pathlib import Path
import tarfile,hashlib,json,sys
shared=Path('/media/psf/ssd_dock/MyRustProjects/dartforge/target/current-desktop-validation');archive=shared/'source-snapshot.tar.gz'
assert hashlib.sha256(archive.read_bytes()).hexdigest()=='f47727d322c631dbb64a3e41ef971e7eddb0301871110dbc3f6abe0129c33e0c'
with tarfile.open(archive) as source:source.extractall(sys.argv[1],filter='data')
rows=json.loads((shared/'source-files.json').read_text())
for row in rows:assert hashlib.sha256((Path(sys.argv[1])/row['path']).read_bytes()).hexdigest()==row['sha256'],row['path']
print('Verified',len(rows),'compiler source files')
PY
chown -R isaque:isaque "$snapshot"
printf '%s\n' "$snapshot" > /tmp/forge-current-desktop-linux-snapshot
ps -eo pid,ppid,etime,comm,args | grep -E 'cargo|rustc|dartforge' | head -n 12 || true
du -sh /home/isaque/projects/dartforge/target/native-validation
