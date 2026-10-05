class A {
}
augment class A {
  void _m([int? a]) {}
}
class B implements A {
  void _m([int? a]) {}
}
f() => A()._m(0);
