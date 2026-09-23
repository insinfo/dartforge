void f(A x) {
  if (x case (A,) _) {}
//           ^^^^
// [diag.patternNeverMatchesValueType] The matched value type 'A' can never match the required type '(A,)'.
}

class A {}
