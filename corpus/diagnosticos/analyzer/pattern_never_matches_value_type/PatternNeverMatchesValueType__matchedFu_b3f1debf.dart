import 'dart:async';

void f(Future<(int,)> x) {
  if (x case FutureOr<(String,)> _) {}
//           ^^^^^^^^^^^^^^^^^^^
// [diag.patternNeverMatchesValueType] The matched value type 'Future<(int,)>' can never match the required type 'FutureOr<(String,)>'.
}
