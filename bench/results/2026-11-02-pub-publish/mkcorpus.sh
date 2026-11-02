#!/bin/bash
# Builds a deflate corpus: inputs plus `gzip.encode` of the Dart 3.6.2 VM.
SP=/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad
DART=/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart
C=$SP/pp/corpus
rm -rf "$C"; mkdir -p "$C"
cat > "$SP/pp/bin/gzall.dart" <<'EOF'
import 'dart:io';
void main(List<String> a) {
  for (final f in Directory(a[0]).listSync()) {
    if (f is! File || f.path.endsWith('.gz')) continue;
    File('${f.path}.gz').writeAsBytesSync(gzip.encode(f.readAsBytesSync()));
  }
}
EOF
n=0
for d in "$SP/src/pub/lib" "$SP/src/pub/test" "$SP/pp/src/tar-32ceb55e673141abff4e84b99483fe5eb881c291" "$SP/pp/zlib362" /Volumes/ssd_dock/tools/dart/pub-cache/hosted/pub.dev/path-1.9.1 /Volumes/ssd_dock/tools/dart/pub-cache/hosted/pub.dev/source_span-1.10.0 /Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib/core /Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib/_internal/vm; do
  n=$((n+1))
  tar -cf "$C/t$n.tar" -C "$(dirname "$d")" "$(basename "$d")" 2>/dev/null
done
head -c 300000 /dev/urandom > "$C/random"
head -c 100 /dev/urandom > "$C/random_small"
python3 -I -c "import sys; open(sys.argv[1],'wb').write(b'abc'*200000)" "$C/rep"
python3 -I -c "import sys; open(sys.argv[1],'wb').write(bytes(range(256))*3000)" "$C/ramp"
printf 'hello hello hello' > "$C/hello"
: > "$C/empty"
printf 'a' > "$C/one"
cp /Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart "$C/dartexe"
cp /Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/snapshots/kernel_worker.dart.snapshot "$C/snapshot"
"$DART" run "$SP/pp/bin/gzall.dart" "$C"
ls -l "$C" | head -50
