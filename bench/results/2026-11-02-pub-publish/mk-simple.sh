#!/bin/bash
# Creates the `simple` fixture package.
SP=/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad
d=$SP/pp/pk/simple
rm -rf "$d"; mkdir -p "$d/lib"
cd "$d" || exit 1
printf 'name: simple\nversion: 1.0.0\ndescription: A simple package for testing the publish command output.\nhomepage: https://example.com\nenvironment:\n  sdk: ^3.5.0\n' > pubspec.yaml
printf 'int f() => 1;\n' > lib/simple.dart
printf '# simple\n' > README.md
printf '## 1.0.0\n\n- First.\n' > CHANGELOG.md
printf 'MIT\n' > LICENSE
