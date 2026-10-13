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
