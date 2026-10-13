void f(int x) {
  if (x case <int>[]) {}
//           ^^^^^^^
// [diag.patternNeverMatchesValueType] The matched value type 'int' can never match the required type 'List<int>'.
}
