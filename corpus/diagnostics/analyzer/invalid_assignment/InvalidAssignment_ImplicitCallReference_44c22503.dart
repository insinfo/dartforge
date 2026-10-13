class C {
  void call(int a) {}
}
class D {
  late void Function(int) f;
}

void foo() {
  D()..f = C();
}
