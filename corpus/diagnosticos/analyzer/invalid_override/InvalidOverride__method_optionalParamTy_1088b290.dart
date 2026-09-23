abstract class I {
  m([int? n]);
}
abstract class J {
  m([num? n]);
//^
// [context 1] The member being overridden.
}
abstract class A implements I, J {}
class B extends A {
  m([String? n]) {}
//^
// [diag.invalidOverride][context 1] 'B.m' ('dynamic Function([String?])') isn't a valid override of 'J.m' ('dynamic Function([num?])').
}
