void f(int x) {
  switch (x) {
    case /*1*/ final a:
//                   ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
    case /*2*/ final a:
//  ^^^^
// [diag.deadCode] Dead code.
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
//                   ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
      return;
  }
}
