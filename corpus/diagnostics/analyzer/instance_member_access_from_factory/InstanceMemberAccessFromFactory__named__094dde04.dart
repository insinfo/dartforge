class A {
  void foo() {}

  factory A.make() {
    // ignore:unused_local_variable
    var x = () => foo();
//                ^^^
// [diag.instanceMemberAccessFromFactory] Instance members can't be accessed from a factory constructor.
    throw 0;
  }
}
