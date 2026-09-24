void f() {
  int v;
  for (var t = (v = 0);;) {
//         ^
// [diag.unusedLocalVariable] The value of the local variable 't' isn't used.
    v;
  }
  v;
//^^
// [diag.deadCode] Dead code.
}
