void f(B x) {
  if (x case A<D> _) {}
//           ^^^^
// [diag.patternNeverMatchesValueType] The matched value type 'B' can never match the required type 'A<D>'.
}

final class A<T> {}
final class B extends A<C> {}
final class C {}
final class D {}
