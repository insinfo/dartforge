class A {
  factory A.named() => throw 0;
  A.generative();
}
class B() extends A {
  this : super.named();
//       ^^^^^^^^^^^^^
// [diag.nonGenerativeConstructor] The generative constructor 'A.named()' is expected, but a factory was found.
}
