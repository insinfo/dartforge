abstract class A {
  void m(int i);
}
abstract class B {
  void m(String s);
}
abstract class C {
  void n(String s);
}
abstract class D implements A, B, C {}
//             ^
// [diag.inconsistentInheritance] Superinterfaces don't have a valid override for 'm': A.m (void Function(int)), B.m (void Function(String)).
