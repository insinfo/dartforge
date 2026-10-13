Object f(bool x) {
  return switch (x) {
    false => 0,
    true => 1,
    false => 2,
//        ^^
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
  };
}
