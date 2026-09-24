void f(A<D> x) {
  if (x case B _) {}
//           ^
// [diag.patternNeverMatchesValueType] The matched value type 'A<D>' can never match the required type 'B'.
}

class A<T> {}
final class B extends A<C> {}
final class C {}
final class D {}
