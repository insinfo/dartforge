void f(Object? x) {
  if (x case A<int>(_foo: var bar)) {
    bar;
  }
}

abstract class A<T> {
  abstract T _foo;
}
