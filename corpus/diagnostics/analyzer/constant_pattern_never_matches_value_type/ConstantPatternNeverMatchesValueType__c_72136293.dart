void f(A<int> x) {
  if (x case const A<num>()) {}
//           ^^^^^^^^^^^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'A<int>' can never be equal to this constant of type 'A<num>'.
}

class A<T> {
  const A();
}
