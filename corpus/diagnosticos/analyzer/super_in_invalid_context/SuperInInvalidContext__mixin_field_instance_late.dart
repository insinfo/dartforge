class A {
  int get foo => 0;
}

mixin M on A {
  late var f = super.foo;
}
