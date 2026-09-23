void f(B x) {
  if (x case const A()) {}
//           ^^^^^^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'B' can never be equal to this constant of type 'A'.
}

class A {
  const A();
}

class B extends A {
  const B();
}
