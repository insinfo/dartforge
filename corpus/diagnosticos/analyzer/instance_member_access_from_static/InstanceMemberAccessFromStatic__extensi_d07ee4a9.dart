extension E on int {
  void foo() {}

  static void bar() {
    foo();
//  ^^^
// [diag.instanceMemberAccessFromStatic] Instance members can't be accessed from a static method.
  }
}
