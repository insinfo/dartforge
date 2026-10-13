void f(bool x) {
  switch (x) {
    case false:
    case true:
    default:
//  ^^^^^^^
// [diag.unreachableSwitchDefault] This default clause is covered by the previous cases.
      break;
  }
}
