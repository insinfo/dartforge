class A {
  m(a, b, [c, d]) {}
//^
// [context 1] The member being overridden.
}
class B extends A {
  m(a, b, [c]) {}
//^
// [diag.invalidOverride][context 1] 'B.m' ('dynamic Function(dynamic, dynamic, [dynamic])') isn't a valid override of 'A.m' ('dynamic Function(dynamic, dynamic, [dynamic, dynamic])').
}
