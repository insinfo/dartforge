class A(final int i) {
  factory A._constructor() => A(7);
//          ^^^^^^^^^^^^
// [diag.unusedElement] The declaration 'A._constructor' isn't referenced.
}
