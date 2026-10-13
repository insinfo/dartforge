class A {
  int foo() => 0;
}

class B1 extends A {}

class B2 extends A {}

abstract class C implements B1, B2 {}

extension type D(C it) implements B1, B2 {}
