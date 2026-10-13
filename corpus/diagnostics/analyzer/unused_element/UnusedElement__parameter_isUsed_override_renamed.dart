class A {
  void _m([int? a]) {}
}
class B implements A {
  void _m([int? b]) {}
}
f() => A()._m(0);
