class A {
  int foo();
  noSuchMethod(im) => 42;
}

class B extends A {
  int foo() => super.foo(); // ref
  noSuchMethod(im) => 87;
}
