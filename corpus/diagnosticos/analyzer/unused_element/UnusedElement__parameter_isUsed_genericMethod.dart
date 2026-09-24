class C {
  void _m<T>([int? x]) {}
}
void foo() {
  C()._m(7);
}
