void f(int x) {
  switch (x) {
    case int():
      break;
    case int():
//  ^^^^
// [diag.deadCode] Dead code.
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
    case int():
//  ^^^^
// [diag.deadCode] Dead code.
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
      break;
//    ^^^^^^
// [diag.deadCode] Dead code.
  }
}
