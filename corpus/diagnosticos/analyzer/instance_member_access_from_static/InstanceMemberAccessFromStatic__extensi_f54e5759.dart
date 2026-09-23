extension E on A {
  int get foo => 0;
}

class A {
  static void bar() {
    foo;
//  ^^^
// [diag.instanceMemberAccessFromStatic] Instance members can't be accessed from a static method.
  }
}
