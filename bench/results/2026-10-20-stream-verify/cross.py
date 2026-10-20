from pathlib import Path
import subprocess,sys
out=Path('/tmp/forge-stream-verify/runtime')
for target in ['linux-arm64','windows-x64']:
 for label in ['before','after']:
  subprocess.run([sys.executable,str(out/'build-cross.py'),target,label],check=True)
subprocess.run([sys.executable,str(out/'inspect-ir.py')],check=True)
subprocess.run([sys.executable,str(out/'inspect-sdk.py')],check=True)
