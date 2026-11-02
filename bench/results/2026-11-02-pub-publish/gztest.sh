#!/bin/bash
# Compares Dart's GZipCodec output with classic zlib (Python) on one tar.
SP=/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad
DART=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart
W=$SP/pp/gz
mkdir -p "$W"
cat > "$SP/pp/bin/gz.dart" <<'EOF'
import 'dart:io';
void main(List<String> a) {
  final bytes = File(a[0]).readAsBytesSync();
  File(a[1]).writeAsBytesSync(gzip.encode(bytes));
}
EOF
src=${1:-$SP/src/pub/lib}
tar -cf "$W/in.tar" -C "$(dirname "$src")" "$(basename "$src")"
"$DART" run "$SP/pp/bin/gz.dart" "$W/in.tar" "$W/dart.gz"
python3 -I -c "
import zlib,sys
d=open(sys.argv[1],'rb').read()
c=zlib.compressobj(6,zlib.DEFLATED,31,8)
o=c.compress(d)+c.flush()
open(sys.argv[2],'wb').write(o)
print(zlib.ZLIB_VERSION, zlib.ZLIB_RUNTIME_VERSION)
" "$W/in.tar" "$W/py.gz"
ls -l "$W/in.tar" "$W/dart.gz" "$W/py.gz"
cmp "$W/dart.gz" "$W/py.gz" && echo SAME
xxd "$W/dart.gz" | head -1
xxd "$W/py.gz" | head -1
