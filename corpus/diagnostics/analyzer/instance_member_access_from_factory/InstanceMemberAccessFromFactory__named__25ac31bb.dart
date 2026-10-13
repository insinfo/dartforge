class A {
  int get foo => 0;

  factory A.make() {
    void f() {
      foo;
//    ^^^
// [diag.instanceMemberAccessFromFactory] Instance members can't be accessed from a factory constructor.
    }
    f();
    throw 0;
  }
}
