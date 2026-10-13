class A {
  int get foo => 0;

  static void bar() {
    // ignore:unused_local_variable
    var x = () => foo;
//                ^^^
// [diag.instanceMemberAccessFromStatic] Instance members can't be accessed from a static method.
  }
}
