class A {
  void foo() {}
}

class B extends A {
  static void bar() {
    foo();
//  ^^^
// [diag.instanceMemberAccessFromStatic] Instance members can't be accessed from a static method.
  }
}
