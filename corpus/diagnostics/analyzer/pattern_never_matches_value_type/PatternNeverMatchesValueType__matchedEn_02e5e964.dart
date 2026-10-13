void f<T>(E<T>? x) {
  if (x case E<String> _) {}
//           ^^^^^^^^^
// [diag.patternNeverMatchesValueType] The matched value type 'E<T>?' can never match the required type 'E<String>'.
}

enum E<T> { v1<int>(), v2<double>() }
