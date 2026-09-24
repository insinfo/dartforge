void f(B x) {
  if (x case const A<int>()) {}
//           ^^^^^^^^^^^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'B' can never be equal to this constant of type 'A<int>'.
}

class A<T> {
  const A();
}

class B extends A<int> {
  const B();
}
