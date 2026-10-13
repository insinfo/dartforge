import 'dart:async';
//     ^^^^^^^^^^^^
// [diag.unusedImport] Unused import: 'dart:async'.

class A {
  A.foo();
}

void f() {
  A.foo();
}
