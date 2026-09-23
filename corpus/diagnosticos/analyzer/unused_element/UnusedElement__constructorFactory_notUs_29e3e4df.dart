class A {
  factory A._factory() => A();
//          ^^^^^^^^
// [diag.unusedElement] The declaration 'A._factory' isn't referenced.
  A();
}
