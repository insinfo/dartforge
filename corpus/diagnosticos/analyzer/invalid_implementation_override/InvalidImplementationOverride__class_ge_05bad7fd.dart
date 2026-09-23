class A<T> {
  void foo<U>(covariant Object a, U b) {}
}
class B extends A<int> {}
