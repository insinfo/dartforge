abstract class A {
  int m();
}
abstract class B {
  String m();
}
mixin M implements A, B {}
//    ^
// [diag.inconsistentInheritance] Superinterfaces don't have a valid override for 'm': A.m (int Function()), B.m (String Function()).
