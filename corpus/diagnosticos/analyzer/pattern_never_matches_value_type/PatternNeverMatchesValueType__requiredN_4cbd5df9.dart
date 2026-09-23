void f(A x) {
  if (x case B? _) {}
//           ^^
// [diag.patternNeverMatchesValueType] The matched value type 'A' can never match the required type 'B?'.
}

final class A {}
final class B {}
