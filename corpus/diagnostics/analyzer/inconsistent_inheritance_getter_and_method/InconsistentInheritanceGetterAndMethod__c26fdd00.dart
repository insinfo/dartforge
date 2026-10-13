class A {
  void foo(String _) {}
}

abstract interface class I {
  int get foo => 1;
}

class C extends A implements I {}

augment class C {
  int foo = 2;
//    ^^^
// [diag.inconsistentInheritanceGetterAndMethod] 'foo' is inherited as a getter (from 'I') and also a method (from 'A').
}
