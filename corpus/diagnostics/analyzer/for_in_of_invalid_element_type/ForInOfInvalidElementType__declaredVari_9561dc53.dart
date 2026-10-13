class C {
  void call<T>(T p) {}
}
void foo(C c) {
  for (void Function(int) f in [c]) {
    f;
  }
}
