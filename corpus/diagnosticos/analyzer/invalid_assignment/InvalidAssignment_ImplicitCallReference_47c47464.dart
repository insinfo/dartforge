class C<T> {
  C(T a);
  void call<U>(T t, U u) {}
}

void Function(int, String) f = C(7);
