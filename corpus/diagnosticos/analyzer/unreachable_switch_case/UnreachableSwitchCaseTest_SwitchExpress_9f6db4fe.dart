Object f(bool x) {
  return switch (x) {
    _ => 0,
    true => 1,
//  ^^^^^^^^^
// [diag.deadCode] Dead code.
//       ^^
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
    false => 2,
//  ^^^^^^^^^^
// [diag.deadCode] Dead code.
//        ^^
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
  };
}
