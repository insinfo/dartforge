void f() {
  int v;
  for (v = 0;;) {
    v;
  }
  v;
//^^
// [diag.deadCode] Dead code.
}
