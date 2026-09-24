abstract class A {
  void m(int i);
}
mixin B {
  void m(String s);
}
abstract class B2 extends Object with B {}
abstract class C implements A, B2 {}
//             ^
// [diag.inconsistentInheritance] Superinterfaces don't have a valid override for 'm': A.m (void Function(int)), B.m (void Function(String)).
