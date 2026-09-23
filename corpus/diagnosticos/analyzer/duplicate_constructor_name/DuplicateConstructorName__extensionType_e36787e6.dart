extension type A.foo(int it) {
  A.foo(this.it);
//^^^^^
// [diag.duplicateConstructorName] The constructor with name 'foo' is already defined.
}
