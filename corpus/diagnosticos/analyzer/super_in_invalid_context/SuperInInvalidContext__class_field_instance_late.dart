class A {
  int foo() => 0;
}

class B extends A {
  late var f = super.foo();
}
