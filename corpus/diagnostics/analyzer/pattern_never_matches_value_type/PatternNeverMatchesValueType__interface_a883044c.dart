void f(List<A> x) {
  if (x case List<B> _) {}
}

final class A {}
final class B extends A {}
