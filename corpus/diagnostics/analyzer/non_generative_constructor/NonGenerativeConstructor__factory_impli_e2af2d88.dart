class A {
  factory A() => throw 0;
  A.named();
}
class B() extends A {
  this;
//^^^^
// [diag.nonGenerativeConstructor] The generative constructor 'A()' is expected, but a factory was found.
}
