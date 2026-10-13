void f(A x) {
  if (x case (0)) {}
//            ^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'A' can never be equal to this constant of type 'int'.
}

class A {}
