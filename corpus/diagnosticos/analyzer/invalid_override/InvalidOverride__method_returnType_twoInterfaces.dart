abstract class I {
  int m();
//    ^
// [context 1] The member being overridden.
}
abstract class J {
  num m();
}
abstract class A implements I, J {}
class B extends A {
  String m() => '';
//       ^
// [diag.invalidOverride][context 1] 'B.m' ('String Function()') isn't a valid override of 'I.m' ('int Function()').
}
