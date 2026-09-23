void f(A x) {
  if (x case R<String> _) {}
//           ^^^^^^^^^
// [diag.patternNeverMatchesValueType] The matched value type 'A<dynamic>' can never match the required type 'R<String>'.
}

enum A<T> implements R<T> {
  v1<int>(),
  v2<double>(),
}

class R<T> {}
