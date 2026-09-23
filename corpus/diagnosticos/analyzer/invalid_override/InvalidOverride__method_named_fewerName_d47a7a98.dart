class A {
  m({a, b}) {}
//^
// [context 1] The member being overridden.
}
class B extends A {
  m({a}) {}
//^
// [diag.invalidOverride][context 1] 'B.m' ('dynamic Function({dynamic a})') isn't a valid override of 'A.m' ('dynamic Function({dynamic a, dynamic b})').
}
