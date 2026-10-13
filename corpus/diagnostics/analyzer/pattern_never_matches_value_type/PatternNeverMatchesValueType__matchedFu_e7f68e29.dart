import 'dart:async';

void f(FutureOr<(int,)> x) {
  if (x case Future<(String,)> _) {}
//           ^^^^^^^^^^^^^^^^^
// [diag.patternNeverMatchesValueType] The matched value type 'FutureOr<(int,)>' can never match the required type 'Future<(String,)>'.
}
