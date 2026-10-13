class A {
  void foo() {}
}

class B {
  set foo(int _) {}
}

abstract class C implements A, B {
  set foo(int _) {}
//    ^^^
// [diag.conflictingFieldAndMethod] Class 'C' can't define field 'foo' and have method 'A.foo' with the same name.
}
