set -euo pipefail
# Resolve from the relocated SDK, never setup-dart's development tree.
test -f "$dist/lib/dart-sdk/sdk_packages.yaml"
test -f "$dist/lib/dart-sdk/pkg/_macros/pubspec.yaml"
[ "$(tr -d '\r\n' < "$dist/lib/dart-sdk/version")" = '3.6.2' ]
mkdir "$d/sdk-package-smoke"
cat > "$d/sdk-package-smoke/pubspec.yaml" <<'EOF'
name: packed_sdk_smoke
environment:
  sdk: '>=3.6.0 <4.0.0'
dependencies:
  _macros:
    sdk: dart
EOF
if [ "$RUNNER_OS" = Windows ]; then
  PUB_CACHE="$d/sdk-package-cache" "$dist/bin/$exe" pub get --offline --sdk "$dist/lib/dart-sdk/lib" --directory "$d/sdk-package-smoke"
else
  env -i PATH=/usr/bin:/bin HOME="$d/home" PUB_CACHE="$d/sdk-package-cache" "$dist/bin/$exe" pub get --offline --sdk "$dist/lib/dart-sdk/lib" --directory "$d/sdk-package-smoke"
fi
grep -Fq '"name": "_macros"' "$d/sdk-package-smoke/.dart_tool/package_config.json"
grep -Fq 'lib/dart-sdk/pkg/_macros/' "$d/sdk-package-smoke/.dart_tool/package_config.json"
grep -Fq 'source: sdk' "$d/sdk-package-smoke/pubspec.lock"
declared_version=$(awk '$1 == "version:" { gsub(/["\047\r]/, "", $2); print $2; exit }' "$dist/lib/dart-sdk/pkg/_macros/pubspec.yaml")
locked_version=$(awk '/^  _macros:/ { package = 1; next } package && /^    version:/ { gsub(/["\047\r]/, "", $2); print $2; exit }' "$d/sdk-package-smoke/pubspec.lock")
[ -n "$declared_version" ] && [ "$locked_version" = "$declared_version" ]
