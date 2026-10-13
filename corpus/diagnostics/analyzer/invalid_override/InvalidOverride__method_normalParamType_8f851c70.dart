class A {
  m(int a) {}
//^
// [context 1] The member being overridden.
}
class B extends A {
  m(String a) {}
//^
// [diag.invalidOverride][context 1] 'B.m' ('dynamic Function(String)') isn't a valid override of 'A.m' ('dynamic Function(int)').
}
