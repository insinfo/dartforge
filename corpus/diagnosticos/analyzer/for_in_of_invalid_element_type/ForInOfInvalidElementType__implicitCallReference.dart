class C {
  void call(int a) {}
}
void foo(Iterable<C> iterable) {
  void Function(int) f;
  for (f in iterable) {
    f;
  }
}
