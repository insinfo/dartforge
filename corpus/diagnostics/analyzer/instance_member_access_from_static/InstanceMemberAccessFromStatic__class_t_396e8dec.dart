class A {
  set foo(int _) {}

  static void bar() {
    foo = 0;
//  ^^^
// [diag.instanceMemberAccessFromStatic] Instance members can't be accessed from a static method.
  }
}
