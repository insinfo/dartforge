mixin A {
  void m(int i);
}
abstract class B {
  void m(String s);
}
abstract class B2 extends B {}
abstract class C extends Object with A implements B2 {}
//             ^
// [diag.inconsistentInheritance] Superinterfaces don't have a valid override for 'm': A.m (void Function(int)), B.m (void Function(String)).
