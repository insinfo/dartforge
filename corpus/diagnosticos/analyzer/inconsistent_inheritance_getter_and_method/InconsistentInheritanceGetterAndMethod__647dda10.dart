class A {
  void foo(String _) {}
}

abstract interface class I {
  int get foo => 1;
}

abstract class C extends A implements I {}
//             ^
// [diag.inconsistentInheritanceGetterAndMethod] 'foo' is inherited as a getter (from 'I') and also a method (from 'A').

augment abstract class C {}
