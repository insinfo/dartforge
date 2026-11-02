#!/bin/bash
# More crafted packages: workspaces and the rarer validator paths.
SP=/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad
PK=$SP/pp/pk
set -e
pkg() { rm -rf "$PK/$1"; mkdir -p "$PK/$1/lib"; printf '%s\n' "$2" > "$PK/$1/pubspec.yaml"; }
std() {
  printf 'MIT\n' > "$PK/$1/LICENSE"
  printf '# %s\n' "$1" > "$PK/$1/README.md"
  printf '## 1.0.0\n\n- First.\n' > "$PK/$1/CHANGELOG.md"
  printf 'int f() => 1;\n' > "$PK/$1/lib/$(basename "$1").dart"
}
head='version: 1.0.0
description: A crafted package for the publish tests of the dartforge tool.
homepage: https://example.com
environment:
  sdk: ^3.5.0'

# Workspace: a root and a member.
rm -rf "$PK/ws"
pkg ws "name: ws_root
$head
workspace:
  - pkgs/a"
std ws
pkg ws/pkgs/a "name: a
$head
resolution: workspace
dependencies:
  foo: ^1.0.0"
std ws/pkgs/a
printf "import 'package:foo/foo.dart';\nint g() => 1;\n" > "$PK/ws/pkgs/a/lib/g.dart"

# Hidden pubspec, invalid ignore pattern, empty directory, no version in the
# changelog, overrides in pubspec_overrides.yaml.
pkg hidden "name: hidden
$head"
std hidden
printf 'pubspec.yaml\n[bad\nignored\\\n' > "$PK/hidden/.pubignore"
mkdir -p "$PK/hidden/empty" "$PK/hidden/doc"
printf '## 0.9.0\n' > "$PK/hidden/CHANGELOG.md"
pkg overrides "name: overrides
$head
dependencies:
  foo: ^1.0.0"
std overrides
printf 'dependency_overrides:\n  foo: 1.0.0\n' > "$PK/overrides/pubspec_overrides.yaml"

# Lazy pubspec errors.
pkg badsecrets "name: badsecrets
$head
false_secrets: lib/x.dart"
std badsecrets
pkg badexec "name: badexec
$head
executables: [a, b]
ignored_advisories: {a: 1}"
std badexec
pkg badexec2 "name: badexec2
$head
executables:
  good: good
  bad: 7
  after: after"
std badexec2
mkdir -p "$PK/badexec2/bin"; printf 'void main() {}\n' > "$PK/badexec2/bin/good.dart"

# A large file with a leak (offsets instead of a highlighted line).
pkg bigleak "name: bigleak
$head"
std bigleak
python3 -I -c '
import sys
body = "// filler line for a large file\n" * 400
open(sys.argv[1], "w").write(body + "final apiKey = \"AIzaSyDG0yD6347wy0i1U4ThqQoEZ0y37ZvFKPM\";\n" + body)
' "$PK/bigleak/lib/big.dart"

# A package whose name matches the server's published versions exactly.
pkg samever "name: relver
version: 2.0.0
description: A crafted package for the publish tests of the dartforge tool.
homepage: https://example.com
environment:
  sdk: ^3.5.0"
std samever
printf '## 2.0.0\n' > "$PK/samever/CHANGELOG.md"
echo fixtures2 ready
