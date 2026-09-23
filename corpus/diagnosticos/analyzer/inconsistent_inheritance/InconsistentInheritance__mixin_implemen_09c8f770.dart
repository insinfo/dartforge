abstract class A {
  void m();
}
abstract class B {
  void m(int y);
}
mixin M implements A, B {}
//    ^
// [diag.inconsistentInheritance] Superinterfaces don't have a valid override for 'm': A.m (void Function()), B.m (void Function(int)).
