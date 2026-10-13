class A {
  m({a, b}) {}
//^
// [context 1] The member being overridden.
}
class B extends A {
  m({a, c}) {}
//^
// [diag.invalidOverride][context 1] 'B.m' ('dynamic Function({dynamic a, dynamic c})') isn't a valid override of 'A.m' ('dynamic Function({dynamic a, dynamic b})').
}
