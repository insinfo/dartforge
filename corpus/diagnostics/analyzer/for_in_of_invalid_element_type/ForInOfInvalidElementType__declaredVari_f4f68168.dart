class C {
  void call() {}
}
void foo(C c) {
  for (void Function() f in [c]) {
    f;
  }
}
