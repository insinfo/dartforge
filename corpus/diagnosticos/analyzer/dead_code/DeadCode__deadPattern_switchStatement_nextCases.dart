void f(int x) {
  switch (x) {
    case int() || 0:
//             ^^^^
// [diag.deadCode] Dead code.
    case 1:
//  ^^^^
// [diag.deadCode] Dead code.
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
    default:
//  ^^^^^^^
// [diag.deadCode] Dead code.
      break;
  }
}
