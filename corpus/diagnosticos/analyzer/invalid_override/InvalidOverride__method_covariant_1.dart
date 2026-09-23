abstract class A<T> {
  A<U> foo<U>(covariant A<Map<T, U>> a);
}

abstract class B<U, T> extends A<T> {
  B<U, V> foo<V>(B<U, Map<T, V>> a);
}
