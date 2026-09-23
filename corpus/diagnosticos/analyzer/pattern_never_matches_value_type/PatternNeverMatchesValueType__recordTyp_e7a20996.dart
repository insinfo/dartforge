void f((int,) x) {
  if (x case (int, String) _) {}
//           ^^^^^^^^^^^^^
// [diag.patternNeverMatchesValueType] The matched value type '(int,)' can never match the required type '(int, String)'.
}
