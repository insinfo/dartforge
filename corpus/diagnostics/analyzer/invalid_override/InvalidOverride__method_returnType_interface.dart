class A {
  int m() { return 0; }
//    ^
// [context 1] The member being overridden.
}
class B implements A {
  String m() { return 'a'; }
//       ^
// [diag.invalidOverride][context 1] 'B.m' ('String Function()') isn't a valid override of 'A.m' ('int Function()').
}
