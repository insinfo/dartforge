abstract class A {
  R foo<R>(VA<R> v);
}

abstract class B implements A {
  R foo<R>(covariant VB<R> v);
}

abstract class VA<T> {}

abstract class VB<T> implements VA<T> {}
