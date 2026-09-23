abstract class A {
  int m();
//    ^
// [context 1] The member being overridden.
}
abstract class B implements A {
}
class C implements B {
  String m() { return 'a'; }
//       ^
// [diag.invalidOverride][context 1] 'C.m' ('String Function()') isn't a valid override of 'A.m' ('int Function()').
}
