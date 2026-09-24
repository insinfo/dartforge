void f(A<B> x) {
  if (x case A<C> _) {}
//           ^^^^
// [diag.patternNeverMatchesValueType] The matched value type 'A<B>' can never match the required type 'A<C>'.
}

final class A<T> {}
final class B {}
final class C {}
