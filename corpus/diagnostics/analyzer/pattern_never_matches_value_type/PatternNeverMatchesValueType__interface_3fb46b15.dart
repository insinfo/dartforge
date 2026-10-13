void f(List<A> x) {
  if (x case List<B> _) {}
//           ^^^^^^^
// [diag.patternNeverMatchesValueType] The matched value type 'List<A>' can never match the required type 'List<B>'.
}

final class A {}
final class B {}
