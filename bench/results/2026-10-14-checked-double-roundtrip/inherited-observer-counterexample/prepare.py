from pathlib import Path
import shutil,hashlib,json
root=Path.cwd();out=root/'target/checked-double-roundtrip/inherited-observer';sdk=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk');copy=out/'sdk';copy.mkdir()
shutil.copytree(sdk/'lib',copy/'lib')
for n in ['version','revision']:shutil.copy2(sdk/n,copy/n)
p=copy/'lib/_internal/vm/lib/object_patch.dart';original=p.read_text();old='throw new NoSuchMethodError.withInvocation(this, invocation);';assert original.count(old)==1;p.write_text(original.replace(old,'return _getHash(this);'))
entry=out/'main.dart';entry.write_text("""@pragma('vm:never-inline')
Object roundTrip(dynamic value) {
  final double scalar = value;
  return scalar;
}
void main() {
  dynamic original = 1.25;
  dynamic returned = roundTrip(original);
  print(original.missingMethod() == returned.missingMethod());
}
""")
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();(out/'inputs.json').write_text(json.dumps({'entry_sha256':sha(entry),'SDK_original_Object_patch_sha256':sha(sdk/'lib/_internal/vm/lib/object_patch.dart'),'SDK_selected_Object_patch_sha256':sha(p),'SDK_version':(copy/'version').read_text().strip(),'replacement':{'old':old,'new':'return _getHash(this);'},'purpose':'Modified selected Object inherited noSuchMethod can observe numeric box identity via native Object_getHash. Default VM SDK is not a modified-SDK execution oracle.'},indent=2)+'\n');print('SDKcopyready',flush=True)
