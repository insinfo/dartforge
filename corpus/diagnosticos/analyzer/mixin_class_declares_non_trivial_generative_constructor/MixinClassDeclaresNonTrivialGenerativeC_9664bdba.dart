mixin class A {
  A() {}
//^
// [diag.mixinClassDeclaresNonTrivialGenerativeConstructor] The mixin class 'A' can't declare a non-trivial generative constructor.
}
class B with A {}
