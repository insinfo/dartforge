class A {
  int m() { return 0; }
//    ^
// [context 1] The member being overridden.
}
class B extends A {
}
class C extends B {
  String m() { return 'a'; }
//       ^
// [diag.invalidOverride][context 1] 'C.m' ('String Function()') isn't a valid override of 'A.m' ('int Function()').
}
