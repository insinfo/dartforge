mixin class A {
  void _foo() {}
//     ^^^^
// [diag.unusedElement] The declaration '_foo' isn't referenced.
}

mixin class B {
  void _foo() {}
//     ^^^^
// [diag.unusedElement] The declaration '_foo' isn't referenced.
}

class C extends Object with A, B {}
