class A {
  int get foo => 0;
}

class B {
  set foo(String _) {}
}

abstract class X implements A, B {}
