class A<T> {
  void remove(T x) {}
}

mixin M<U> on A<U> {
  void remove(Object? x) {
    super.remove(x as U);
  }
}

class X<T> = A<T> with M<T>;
