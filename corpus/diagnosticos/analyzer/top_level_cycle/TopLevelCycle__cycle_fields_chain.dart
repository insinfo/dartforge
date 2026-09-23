class A {
  static final a = b.c;
//             ^
// [diag.topLevelCycle] The type of 'a' can't be inferred because it depends on itself through the cycle: a, c.
  static final b = A();
  final c = a;
//      ^
// [diag.topLevelCycle] The type of 'c' can't be inferred because it depends on itself through the cycle: a, c.
}
