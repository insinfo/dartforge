mixin class A {
  A(int foo);
//^
// [diag.mixinClassDeclaresNonTrivialGenerativeConstructor] The mixin class 'A' can't declare a non-trivial generative constructor.
}
class B with A {}
