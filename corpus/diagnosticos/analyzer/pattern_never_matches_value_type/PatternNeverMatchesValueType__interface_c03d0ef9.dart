void f(A x) {
  if (x case R _) {}
//           ^
// [diag.patternNeverMatchesValueType] The matched value type 'A' can never match the required type 'R'.
}

sealed class A {}
final class A2 extends A {}
final class A3 implements A {}
class R {}
