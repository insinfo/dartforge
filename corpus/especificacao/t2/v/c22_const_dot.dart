class A {
  final int x;
  A(this.x);
}

void f() {
  A a = .new(0);
  A b = const .new(0);
  print([a, b]);
}
