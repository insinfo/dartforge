void f() {
  int v;
  if (!true) {
// [diag.deadCode][column 14][length 25] Dead code.
    // not assigned
  } else {
    v = 0;
  }
  v;
}
