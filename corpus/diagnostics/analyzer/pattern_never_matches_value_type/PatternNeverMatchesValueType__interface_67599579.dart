void f(A x) {
  if (x case R _) {}
//           ^
// [diag.patternNeverMatchesValueType] The matched value type 'A' can never match the required type 'R'.
}

final class A {}
class R {}
