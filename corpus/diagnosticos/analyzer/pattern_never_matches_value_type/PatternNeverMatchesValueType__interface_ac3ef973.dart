import 'dart:async';

final class A {}
class B {}

void f(A x) {
  if (x case FutureOr<B> _) {}
//           ^^^^^^^^^^^
// [diag.patternNeverMatchesValueType] The matched value type 'A' can never match the required type 'FutureOr<B>'.
}
