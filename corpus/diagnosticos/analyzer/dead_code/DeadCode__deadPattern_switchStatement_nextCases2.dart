void f(int x) {
  switch (x) {
    case int() || 42:
//             ^^^^^
// [diag.deadCode] Dead code.
    case int() || 1:
//  ^^^^
// [diag.deadCode] Dead code.
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
    case 2:
//  ^^^^
// [diag.deadCode] Dead code.
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
      break;
  }
}
