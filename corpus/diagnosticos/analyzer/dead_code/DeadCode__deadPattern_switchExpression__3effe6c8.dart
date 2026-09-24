Object f(int x) {
  return switch (x) {
    int() || 0 => 0,
//        ^^^^
// [diag.deadCode] Dead code.
  };
}
