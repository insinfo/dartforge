class C {
  void call<T>(T p) {}
}
void foo(Iterable<C> iterable) {
  void Function(int) f;
  for (f in iterable) {
    f;
  }
}
