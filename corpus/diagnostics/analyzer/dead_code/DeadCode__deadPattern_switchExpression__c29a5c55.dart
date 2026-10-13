Object f(int x) {
  return switch (x) {
    int() || 0 => 0,
//        ^^^^
// [diag.deadCode] Dead code.
    int() => 1,
//  ^^^^^^^^^^
// [diag.deadCode] Dead code.
//        ^^
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
    _ => 2,
//  ^^^^^^
// [diag.deadCode] Dead code.
//    ^^
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
  };
}
