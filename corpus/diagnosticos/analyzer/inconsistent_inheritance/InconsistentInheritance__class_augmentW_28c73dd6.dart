abstract class I {
  String foo();
}

mixin M {
  int foo() => 0;
}

class A implements I {}
//    ^
// [diag.inconsistentInheritance] Superinterfaces don't have a valid override for 'foo': M.foo (int Function()), I.foo (String Function()).

augment class A with M {}
