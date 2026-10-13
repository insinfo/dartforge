class A {
  int m() { return 0; }
//    ^
// [context 1] The member being overridden.
}
class B extends A {
  void m() {}
//     ^
// [diag.invalidOverride][context 1] 'B.m' ('void Function()') isn't a valid override of 'A.m' ('int Function()').
}
