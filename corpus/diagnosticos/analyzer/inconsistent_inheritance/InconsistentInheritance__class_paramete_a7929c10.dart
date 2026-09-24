abstract class A {
  void m(int i);
}
abstract class B {
  void m(String s);
}
abstract class C extends B implements A {}
//             ^
// [diag.inconsistentInheritance] Superinterfaces don't have a valid override for 'm': B.m (void Function(String)), A.m (void Function(int)).
