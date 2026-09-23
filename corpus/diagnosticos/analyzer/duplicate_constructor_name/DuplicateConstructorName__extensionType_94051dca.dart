extension type A(int it) {
  factory A.foo(int it) => A(it);
  factory foo(int it) => A(it);
//^^^^^^^^^^^
// [diag.duplicateConstructorName] The constructor with name 'foo' is already defined.
}
