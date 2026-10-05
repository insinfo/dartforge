class A {
  final int a;
  A(this.a);
  A.named(this.a);
}
augment class A {
  augment A(int x);
  augment factory A.f() = A.named;
  augment void m() {}
}
