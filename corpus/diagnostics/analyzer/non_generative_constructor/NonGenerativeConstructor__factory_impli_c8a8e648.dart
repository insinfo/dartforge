class A {
  factory A() => throw 0;
  A.named();
}
class B extends A {
  new foo();
//^^^^^^^
// [diag.nonGenerativeConstructor] The generative constructor 'A()' is expected, but a factory was found.
}
