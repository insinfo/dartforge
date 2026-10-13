class A {
  void foo() {}

  factory A() {
    foo();
//  ^^^
// [diag.instanceMemberAccessFromFactory] Instance members can't be accessed from a factory constructor.
    throw 0;
  }
}
