import 'dart:async';

void f((int,) x) {
  if (x case FutureOr<(String,)> _) {}
//           ^^^^^^^^^^^^^^^^^^^
// [diag.patternNeverMatchesValueType] The matched value type '(int,)' can never match the required type 'FutureOr<(String,)>'.
}
