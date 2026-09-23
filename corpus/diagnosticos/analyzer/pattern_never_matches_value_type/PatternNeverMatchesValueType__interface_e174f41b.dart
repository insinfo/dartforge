void f(A<B> x) {
  if (x case A<C> _) {}
}

final class A<T> {}
final class B {}
final class C extends B {}
