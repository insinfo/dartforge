void f(bool x) {
  switch (x) {
    case false:
    case true:
    case false:
//  ^^^^
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
      break;
  }
}
