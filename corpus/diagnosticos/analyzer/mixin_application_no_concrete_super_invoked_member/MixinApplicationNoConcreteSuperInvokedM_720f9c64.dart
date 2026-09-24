abstract class A {
  void foo();
}

mixin M on A {}

abstract class X extends A with M {}
