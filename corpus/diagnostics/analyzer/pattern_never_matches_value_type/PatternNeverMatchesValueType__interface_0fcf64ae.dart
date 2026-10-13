void f(A<C> x) {
  if (x case B _) {}
}

class A<T> {}
final class B extends A<C> {}
final class C {}
