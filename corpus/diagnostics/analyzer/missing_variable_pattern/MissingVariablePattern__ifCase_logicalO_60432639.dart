void f(int x) {
  if (x case final a || final a) {}
//                 ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
//                   ^^^^^^^^^^
// [diag.deadCode] Dead code.
//                            ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
