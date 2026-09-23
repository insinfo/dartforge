void f(A x) {
  if (x case R _) {}
//           ^
// [diag.patternNeverMatchesValueType] The matched value type 'A' can never match the required type 'R'.
}

class A {}
sealed class R {}
final class R1 extends R {}
final class R2 extends R {}
