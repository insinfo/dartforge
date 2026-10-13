class A {
  A() => A();
//    ^^^^^^^
// [diag.returnInGenerativeConstructor] Constructors can't return values.
}
