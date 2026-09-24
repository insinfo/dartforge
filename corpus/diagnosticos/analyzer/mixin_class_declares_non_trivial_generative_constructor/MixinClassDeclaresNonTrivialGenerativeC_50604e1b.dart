mixin class A {
  A() : this.named();
//^
// [diag.mixinClassDeclaresNonTrivialGenerativeConstructor] The mixin class 'A' can't declare a non-trivial generative constructor.
  A.named();
}
class B with A {}
