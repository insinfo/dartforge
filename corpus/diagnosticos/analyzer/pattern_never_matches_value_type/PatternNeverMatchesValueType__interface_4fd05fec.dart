void f(A x) {
  if (x case C _) {}
//           ^
// [diag.patternNeverMatchesValueType] The matched value type 'A' can never match the required type 'C'.
}

extension type A(B _) {}

class B {}

final class C {}
