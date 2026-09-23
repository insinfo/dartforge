class A {
  static final x = y + 1;
//             ^
// [diag.topLevelCycle] The type of 'x' can't be inferred because it depends on itself through the cycle: x, y.
  static final y = x + 1;
//             ^
// [diag.topLevelCycle] The type of 'y' can't be inferred because it depends on itself through the cycle: x, y.
}
