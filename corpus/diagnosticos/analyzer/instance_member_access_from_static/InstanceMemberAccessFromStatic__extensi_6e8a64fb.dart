extension E on A {
  void foo() {}
}

class A {
  static void bar() {
    foo();
//  ^^^
// [diag.instanceMemberAccessFromStatic] Instance members can't be accessed from a static method.
  }
}
