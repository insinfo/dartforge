mixin A {
  void _foo() {}
//     ^^^^
// [diag.unusedElement] The declaration '_foo' isn't referenced.
}

mixin B {
  void _foo() {}
//     ^^^^
// [diag.unusedElement] The declaration '_foo' isn't referenced.
}

class C extends Object with A, B {}
