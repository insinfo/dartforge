abstract class A {
  void m(int i);
}
abstract class B {
  void m(String s);
}
mixin M implements A, B {}
//    ^
// [diag.inconsistentInheritance] Superinterfaces don't have a valid override for 'm': A.m (void Function(int)), B.m (void Function(String)).
