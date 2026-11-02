#!/bin/bash
# Creates the crafted packages under $SP/pp/pk and the fake server's
# packages under $SP/pp/server/packages.
SP=/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad
PK=$SP/pp/pk
SRV=$SP/pp/server/packages
set -e

pkg() { # pkg <name> <pubspec text>
  rm -rf "$PK/$1"; mkdir -p "$PK/$1/lib"
  printf '%s\n' "$2" > "$PK/$1/pubspec.yaml"
}
std() { # LICENSE, README, CHANGELOG for 1.0.0 and lib/<name>.dart
  printf 'MIT\n' > "$PK/$1/LICENSE"
  printf '# %s\n' "$1" > "$PK/$1/README.md"
  printf '## 1.0.0\n\n- First.\n' > "$PK/$1/CHANGELOG.md"
  printf 'int f() => 1;\n' > "$PK/$1/lib/$(basename "$1").dart"
}

# --- server packages -------------------------------------------------------
server_pkg() { # server_pkg <name> <version> <pubspec json> [retracted]
  local dir=$SP/pp/srvsrc/$1-$2
  rm -rf "$dir"; mkdir -p "$dir/lib" "$SRV/$1"
  python3 -I -c '
import json,sys
p=json.loads(sys.argv[1])
lines=["name: "+p["name"],"version: "+p["version"]]
env=p.get("environment",{})
if env:
  lines.append("environment:")
  for k,v in env.items(): lines.append("  %s: \"%s\"" % (k,v))
deps=p.get("dependencies",{})
if deps:
  lines.append("dependencies:")
  for k,v in deps.items(): lines.append("  %s: \"%s\"" % (k,v))
open(sys.argv[2],"w").write("\n".join(lines)+"\n")
' "$3" "$dir/pubspec.yaml"
  printf 'int x() => 1;\n' > "$dir/lib/$1.dart"
  (cd "$dir" && tar -czf "$SRV/$1/$2.tar.gz" pubspec.yaml lib)
  if [ -n "$4" ]; then
    python3 -I -c 'import json,sys; p=json.loads(sys.argv[1]); p["_retracted"]=True; print(json.dumps(p))' "$3" > "$SRV/$1/$2.json"
  else
    printf '%s\n' "$3" > "$SRV/$1/$2.json"
  fi
}
rm -rf "$SRV"; mkdir -p "$SRV"
server_pkg foo 1.0.0 '{"name":"foo","version":"1.0.0","environment":{"sdk":">=2.12.0 <4.0.0"}}'
server_pkg foo 1.2.0 '{"name":"foo","version":"1.2.0","environment":{"sdk":">=2.12.0 <4.0.0"}}'
server_pkg foo 2.0.0-dev.1 '{"name":"foo","version":"2.0.0-dev.1","environment":{"sdk":">=2.12.0 <4.0.0"}}'
server_pkg bar 0.3.1 '{"name":"bar","version":"0.3.1","environment":{"sdk":">=2.12.0 <4.0.0"}}'
server_pkg bar 0.4.0 '{"name":"bar","version":"0.4.0","environment":{"sdk":">=2.12.0 <4.0.0"}}' retracted
server_pkg pathdep 3.1.0 '{"name":"pathdep","version":"3.1.0","environment":{"sdk":">=2.12.0 <4.0.0"}}'
# Published versions of packages that are about to be published again.
server_pkg relver 0.9.0 '{"name":"relver","version":"0.9.0","environment":{"sdk":">=2.7.0 <3.0.0"}}'
server_pkg relver 2.0.0 '{"name":"relver","version":"2.0.0","environment":{"sdk":">=2.12.0 <4.0.0"}}'
server_pkg relmajor 1.4.2 '{"name":"relmajor","version":"1.4.2","environment":{"sdk":">=2.12.0 <4.0.0"}}'

# --- simple ----------------------------------------------------------------
pkg simple 'name: simple
version: 1.0.0
description: A simple package for testing the publish command output.
homepage: https://example.com
environment:
  sdk: ^3.5.0'
std simple

# --- bare: no LICENSE/README/CHANGELOG, missing fields ---------------------
pkg bare 'name: bare
environment:
  sdk: ">=3.0.0"'
printf 'int f() => 1;\n' > "$PK/bare/lib/bare.dart"

# --- fields: deprecated fields, bad URLs, executables, typos ---------------
pkg fields 'name: fields
version: 1.0.0
description: 7
homepage: ftp://example.com/home
repository: [1, 2]
documentation: docs.example.com
author: Someone <someone@example.com>
authors:
  - A
transformers: []
web: {compiler: dart2js}
dependecies: {}
dev_dependecy: {}
enviroment: {}
excutables: {}
executables:
  tool: main
  other:
  bad: sub/x
environment:
  sdk: ^3.5.0'
std fields
mkdir -p "$PK/fields/bin" "$PK/fields/tests" "$PK/fields/examples" "$PK/fields/samples" "$PK/fields/tools"
printf 'void main() {}\n' > "$PK/fields/bin/other.dart"
printf 'x\n' > "$PK/fields/tests/a.txt"
printf 'x\n' > "$PK/fields/examples/a.txt"
printf 'x\n' > "$PK/fields/samples/a.txt"
printf 'x\n' > "$PK/fields/tools/a.txt"
printf 'int g() => 2;\n' > "$PK/fields/lib/other.dart"

# --- names: upper-case package name, library name mismatch -----------------
pkg namesUpper 'name: namesUpper
version: 1.0.0
description: A package whose name is not lower case at all, for testing.
repository: https://example.com/repo
environment:
  sdk: ^3.5.0'
std namesUpper
rm "$PK/namesUpper/lib/namesUpper.dart"
printf 'int f() => 1;\n' > "$PK/namesUpper/lib/wrong_name.dart"
mkdir -p "$PK/namesUpper/lib/src"
printf 'int h() => 1;\n' > "$PK/namesUpper/lib/src/h.dart"

# --- docs: README/CHANGELOG variants, dartdoc output ------------------------
pkg docs 'name: docs
version: 1.2.3
description: A package with odd documentation files for the publish tests.
homepage: https://example.com
environment:
  sdk: ^3.5.0'
printf 'MIT\n' > "$PK/docs/COPYING"
printf 'MIT\n' > "$PK/docs/MIT-LICENSE.txt"
printf '# docs \xff\xfe\n' > "$PK/docs/README.txt"
printf '# docs\n' > "$PK/docs/readme.markdown.md"
printf '## 1.0.0\n' > "$PK/docs/changelog.txt"
printf 'int f() => 1;\n' > "$PK/docs/lib/docs.dart"
mkdir -p "$PK/docs/doc/api"
for f in nav.json index.html styles.css dart-logo-small.png client-live-nav.js; do printf 'x\n' > "$PK/docs/doc/api/$f"; done

# --- sdk: SDK constraints ----------------------------------------------------
pkg sdkold 'name: sdkold
version: 1.0.0
description: A package with an old SDK constraint written with an upper bound.
homepage: https://example.com
environment:
  sdk: ">=2.12.0 <3.0.0"'
std sdkold
pkg sdkpre 'name: sdkpre
version: 1.0.0
description: A package that needs a prerelease of the Dart SDK for testing.
homepage: https://example.com
environment:
  sdk: ">=3.6.0-0 <4.0.0"'
std sdkpre

# --- deps: dependency constraints and sources --------------------------------
pkg deps 'name: deps
version: 1.0.0
description: A package with all kinds of dependency constraints for testing.
homepage: https://example.com
environment:
  sdk: ^3.5.0
dependencies:
  foo: any
  bar: 0.3.1
  pathdep:
    path: ../pathdep
dev_dependencies:
  devonly: {path: ../devonly}
dependency_overrides:
  foo: 1.0.0'
std deps
pkg pathdep 'name: pathdep
version: 3.1.0
environment:
  sdk: ^3.5.0'
pkg other 'name: other
version: 0.1.0
environment:
  sdk: ^3.5.0'
pkg devonly 'name: devonly
version: 0.0.1
environment:
  sdk: ^3.5.0'
pkg deps2 'name: deps2
version: 1.0.0
description: A package with more kinds of dependency constraints for testing.
homepage: https://example.com
environment:
  sdk: ^3.5.0
dependencies:
  foo: ">=1.0.0"
  bar: "<0.4.0"
  other: {path: ../other, version: ">0.0.0"}'
std deps2
pkg deps3 'name: deps3
version: 1.0.0
description: A package depending on a prerelease and a range without minimum.
homepage: https://example.com
environment:
  sdk: ^3.5.0
dependencies:
  foo: ">=2.0.0-dev.1 <3.0.0"
  bar: ">0.3.0"'
std deps3

# --- strict: imports of undeclared packages ----------------------------------
pkg strict 'name: strict
version: 1.0.0
description: A package whose imports do not match its declared dependencies.
homepage: https://example.com
environment:
  sdk: ^3.5.0
dependencies:
  foo: ^1.0.0
dev_dependencies:
  bar: ^0.3.0'
std strict
cat > "$PK/strict/lib/strict.dart" <<'EOF'
import 'dart:io';
import 'package:foo/foo.dart';
import 'package:bar/bar.dart';
import 'package:missing/missing.dart';
/// A documented export.
export 'package:strict/src/a.dart';
import 'package:nope';
@Deprecated('x')
import 'package:also_missing/x.dart'
    show y;
import 'src/b.dart';
EOF
mkdir -p "$PK/strict/lib/src" "$PK/strict/test" "$PK/strict/bin" "$PK/strict/tool"
printf 'int a = 1;\n' > "$PK/strict/lib/src/a.dart"
printf 'int b = 1;\n' > "$PK/strict/lib/src/b.dart"
printf "import 'package:bar/bar.dart';\nimport 'package:test_missing/t.dart';\nvoid main() {}\n" > "$PK/strict/test/a_test.dart"
printf "import 'package:bar/bar.dart';\nvoid main() {}\n" > "$PK/strict/bin/main.dart"
printf "import 'package:tool_missing/t.dart';\nvoid main( {}\n" > "$PK/strict/tool/broken.dart"
printf "import \"package:tabbed/x.dart\";\t// tab\nvoid main() {}\n" > "$PK/strict/tool/t.dart"

# --- leaks: secrets ------------------------------------------------------------
pkg leaks 'name: leaks
version: 1.0.0
description: A package that leaks secrets in its files, for the tests here.
homepage: https://example.com
environment:
  sdk: ^3.5.0
false_secrets:
  - /lib/ignored.dart'
std leaks
printf 'final accessKey = "AKIAVBOGPFGGW6HQOSMY";\n' > "$PK/leaks/lib/aws.dart"
printf 'final accessKey = "AKIAVBOGPFGGW6HQOSMY";\n' > "$PK/leaks/lib/ignored.dart"
pkg leaks4 'name: leaks4
version: 1.0.0
description: A package that leaks four secrets in two files for the tests.
homepage: https://example.com
environment:
  sdk: ^3.5.0'
std leaks4
printf 'final apiKey = "AIzaSyDG0yD6347wy0i1U4ThqQoEZ0y37ZvFKPM";\nfinal accessKey = "AKIAVBOGPFGGW6HQOSMY";\n' > "$PK/leaks4/lib/a.dart"
printf -- '-----BEGIN PRIVATE KEY-----\nH0M6xpM2q+53wmsN/eYLdgtjgBd3DBmHtPilCkiFICXyaA8z9LkJ\n-----END PRIVATE KEY-----\n' > "$PK/leaks4/lib/key.pem"
printf 'final apiKey = "AIzaSyCBSJpVO1A2yHOKP627dSmarIrdgvBygjw";\n' > "$PK/leaks4/lib/b.dart"

# --- ignore: .pubignore/.gitignore rules -----------------------------------------
pkg ignore 'name: ignore
version: 1.0.0
description: A package with ignore files at several levels for the tests here.
homepage: https://example.com
environment:
  sdk: ^3.5.0'
std ignore
cd "$PK/ignore"
mkdir -p build sub/deep sub/keep empty onlyhidden web .hidden "sub/pubspec.lock" lib/gen
printf '*.log\nbuild/\n!important.log\nsub/deep/\n/rootonly.txt\n' > .gitignore
printf 'a\n' > a.log
printf 'a\n' > important.log
printf 'a\n' > build/out.txt
printf 'a\n' > sub/x.log
printf 'a\n' > sub/deep/y.txt
printf 'a\n' > sub/keep/z.txt
printf '*.txt\n!keep.txt\n' > sub/.pubignore
printf 'a\n' > sub/keep.txt
printf 'a\n' > sub/drop.txt
printf '*.txt\n' > sub/keep/.gitignore
printf 'a\n' > rootonly.txt
printf 'a\n' > sub/rootonly.txt
printf 'a\n' > onlyhidden/.gitkeep
printf 'a\n' > web/.htaccess
printf 'a\n' > .hidden/x
printf 'a\n' > sub/pubspec.lock/inside.txt
printf 'dependency_overrides: {}\n' > pubspec_overrides.yaml
printf 'a\n' > lib/pubspec_overrides.yaml
printf 'a\n' > lib/pubspec.lock
printf '[bad\n*.g.dart\n' > lib/gen/.gitignore
printf 'a\n' > lib/gen/x.g.dart
printf 'a\n' > lib/gen/y.dart
printf 'a\n' > "lib/gen/with space.dart"
printf '#!/bin/sh\n' > tool.sh; chmod +x tool.sh
mkdir -p "lib/a/very/long/path/name/that/goes/beyond/the/limit/of/one/hundred/bytes/in/the/tar/header"
printf 'a\n' > "lib/a/very/long/path/name/that/goes/beyond/the/limit/of/one/hundred/bytes/in/the/tar/header/file.dart"

# --- devtools, private ----------------------------------------------------------
pkg devtools 'name: devtools
version: 1.0.0
description: A package with a devtools extension folder that is incomplete.
homepage: https://example.com
environment:
  sdk: ^3.5.0'
std devtools
mkdir -p "$PK/devtools/extension/devtools/build"
printf 'x\n' > "$PK/devtools/extension/devtools/notes.txt"
pkg private 'name: private
version: 1.0.0
description: A private package that cannot be published anywhere at all.
homepage: https://example.com
publish_to: none
environment:
  sdk: ^3.5.0'
std private
pkg publishto 'name: publishto
version: 1.0.0
description: A package with its own server to publish to, a local one.
homepage: https://example.com
publish_to: http://127.0.0.1:8765
environment:
  sdk: ^3.5.0'
std publishto
pkg badpublishto 'name: badpublishto
version: 1.0.0
description: A package with a server address that pub cannot publish to.
homepage: https://example.com
publish_to: ftp://example.com
environment:
  sdk: ^3.5.0'
std badpublishto

# --- relver: version numbering against the server --------------------------------
pkg relver 'name: relver
version: 1.0.0
description: A package whose previous versions are on the fake server already.
homepage: https://example.com
environment:
  sdk: ^3.5.0'
std relver
pkg relmajor 'name: relmajor
version: 3.0.0
description: A package that skips a major version on the fake server here.
homepage: https://example.com
environment:
  sdk: ^3.5.0'
std relmajor
printf '## 3.0.0\n' > "$PK/relmajor/CHANGELOG.md"

# --- analyze: a library with an error and a pubspec with a warning -------------
pkg analyze 'name: analyze
version: 1.0.0
description: A package whose library does not analyze cleanly at all here.
homepage: https://example.com
environment:
  sdk: ^3.5.0'
std analyze
printf 'int f() => "not an int";\nvoid unused() { var x = 1; }\n' > "$PK/analyze/lib/analyze.dart"

# --- symlinks --------------------------------------------------------------------
pkg linkdir 'name: linkdir
version: 1.0.0
description: A package with a symbolic link to a directory in its library.
homepage: https://example.com
environment:
  sdk: ^3.5.0'
std linkdir
mkdir -p "$PK/linkdir/real"
ln -s ../real "$PK/linkdir/lib/linked"
pkg linkbroken 'name: linkbroken
version: 1.0.0
description: A package with a broken symbolic link in its library folder.
homepage: https://example.com
environment:
  sdk: ^3.5.0'
std linkbroken
ln -s nowhere.dart "$PK/linkbroken/lib/broken.dart"
pkg linkfile 'name: linkfile
version: 1.0.0
description: A package with a symbolic link to a file in its library folder.
homepage: https://example.com
environment:
  sdk: ^3.5.0'
std linkfile
ln -s linkfile.dart "$PK/linkfile/lib/alias.dart"

# --- git: a package inside a repository ----------------------------------------------
rm -rf "$PK/repo"; mkdir -p "$PK/repo/pkgs"
cd "$PK/repo"
git init -q .
git config user.email t@example.com; git config user.name t
printf '*.secret\nignored_dir/\n' > .gitignore
pkg repo/pkgs/inrepo 'name: inrepo
version: 1.0.0
description: A package inside a git repository, one directory below its root.
homepage: https://example.com
environment:
  sdk: ^3.5.0'
std repo/pkgs/inrepo
printf 'x\n' > pkgs/inrepo/lib/committed.secret
mkdir -p pkgs/inrepo/ignored_dir
printf 'x\n' > pkgs/inrepo/ignored_dir/a.txt
printf 'x\n' > pkgs/inrepo/lib/modified.dart
git add -A >/dev/null; git add -f pkgs/inrepo/lib/committed.secret pkgs/inrepo/ignored_dir/a.txt
git commit -qm init
printf 'int y = 2;\n' > pkgs/inrepo/lib/modified.dart
printf 'x\n' > pkgs/inrepo/lib/untracked.dart
cd "$SP"
echo fixtures ready
