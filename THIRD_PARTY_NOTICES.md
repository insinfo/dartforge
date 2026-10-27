# Third-party notices

DartForge is MIT licensed. The abbreviated formatting of Iterable in
`crates/codegen/src/core.js` adapts the algorithm of
[Dart SDK 3.6.2 core/iterable.dart](https://github.com/dart-lang/sdk/blob/3.6.2/sdk/lib/core/iterable.dart),
Copyright (c) 2011, the Dart project authors. All rights reserved.
The notice below is also preserved in the JS output that includes that runtime.

`corpus/diagnostics/` (the parity scoreboard of `crates/parity`) contains
copies of `tests/language` and excerpts of `pkg/analyzer/test/src/diagnostics`
from the Dart SDK, `pkg/expect` from the same SDK and the `lib/` of the `meta`
1.16.0 package, all by the Dart project authors under the same license below.
The table `crates/diagnostics/src/codes_g.rs` is generated from the message
templates of `analyzer` 6.11.0 and `_fe_analyzer_shared` 76.0.0, likewise.

```text
Copyright 2012, the Dart project authors.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:
    * Redistributions of source code must retain the above copyright
      notice, this list of conditions and the following disclaimer.
    * Redistributions in binary form must reproduce the above
      copyright notice, this list of conditions and the following
      disclaimer in the documentation and/or other materials provided
      with the distribution.
    * Neither the name of Google LLC nor the names of its
      contributors may be used to endorse or promote products derived
      from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

The onEnter rules for `/** ... */` comments and the indentation rules in
`editors/vscode/language-configuration.json` follow the ones of the TypeScript
language configuration built into
[VS Code](https://github.com/microsoft/vscode/blob/main/extensions/typescript-basics/language-configuration.json),
used under the MIT license below. The Dart TextMate grammar
(`editors/vscode/syntaxes/dart.tmLanguage.json`) is original to DartForge.

```text
Copyright (c) 2015 - present Microsoft Corporation

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
