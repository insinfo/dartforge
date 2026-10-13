void f(A x) {
  if (x case R<int> _) {}
//           ^^^^^^
// [diag.patternNeverMatchesValueType] The matched value type 'A' can never match the required type 'R<int>'.
}

enum A implements R<num> { v }
class R<T> {}
