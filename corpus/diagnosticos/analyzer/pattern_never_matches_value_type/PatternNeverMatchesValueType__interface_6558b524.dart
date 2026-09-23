void f(A<num> x) {
  if (x case B _) {}
}

class A<T> {}
final class B extends A<int> {}
