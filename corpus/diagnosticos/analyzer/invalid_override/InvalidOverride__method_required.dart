class A {
  m(a) {}
//^
// [context 1] The member being overridden.
}
class B extends A {
  m(a, b) {}
//^
// [diag.invalidOverride][context 1] 'B.m' ('dynamic Function(dynamic, dynamic)') isn't a valid override of 'A.m' ('dynamic Function(dynamic)').
}
