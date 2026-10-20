set -euo pipefail
snapshot=$(cat /tmp/forge-batch-tables-linux-snapshot)
runuser -l isaque -c "PATH=/opt/llvm-22.1.8/bin:\$PATH DARTFORGE_LLVM_DIR=/opt/llvm-22.1.8 /home/isaque/projects/dartforge/target/native-validation/debug/dartforge-batch-tables run --sdk /opt/dart/3.6.2/dart-sdk/lib '$snapshot/crates/emit_native/tests/fixtures/table_batch/main.dart'" > /tmp/forge-batch-tables-linux-jit-normal.stdout 2> /tmp/forge-batch-tables-linux-jit-normal.stderr
runuser -l isaque -c "PATH=/opt/llvm-22.1.8/bin:\$PATH DARTFORGE_LLVM_DIR=/opt/llvm-22.1.8 DARTFORGE_GC_STRESS=1 /home/isaque/projects/dartforge/target/native-validation/debug/dartforge-batch-tables run --sdk /opt/dart/3.6.2/dart-sdk/lib '$snapshot/crates/emit_native/tests/fixtures/table_batch/main.dart'" > /tmp/forge-batch-tables-linux-jit-stress.stdout 2> /tmp/forge-batch-tables-linux-jit-stress.stderr
python3 - <<'CHECK'
from pathlib import Path
import json
rows=[]
for mode in ['normal','stress']:
 stdout=Path('/tmp/forge-batch-tables-linux-jit-'+mode+'.stdout').read_text();stderr=Path('/tmp/forge-batch-tables-linux-jit-'+mode+'.stderr').read_text();assert stdout=='parent=45\nchild=45\n' and not stderr,(stdout,stderr);rows.append(dict(mode=mode,exit=0,stdout=stdout,stderr=stderr))
print(json.dumps(rows))
CHECK
