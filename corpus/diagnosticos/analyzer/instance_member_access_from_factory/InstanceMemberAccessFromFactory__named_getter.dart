class A {
  int get foo => 0;

  factory A.make() {
    foo;
//  ^^^
// [diag.instanceMemberAccessFromFactory] Instance members can't be accessed from a factory constructor.
    throw 0;
  }
}
