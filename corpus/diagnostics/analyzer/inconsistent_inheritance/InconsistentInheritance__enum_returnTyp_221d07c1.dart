abstract class A {
  int foo();
}

abstract class B {
  String foo();
}

enum E implements A {v}
//   ^
// [diag.inconsistentInheritance] Superinterfaces don't have a valid override for 'foo': A.foo (int Function()), B.foo (String Function()).

augment enum E implements B {}
