class A {
  void _m([int? a]) {}
//              ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
class B implements A {
  void _m([int? a]) {}
}
f() {
  A()._m();
  B()._m(0);
}
