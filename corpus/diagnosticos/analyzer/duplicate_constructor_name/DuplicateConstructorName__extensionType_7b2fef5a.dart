extension type A(int it) {
  A.foo(this.it);
  new foo(this.it);
//^^^^^^^
// [diag.duplicateConstructorName] The constructor with name 'foo' is already defined.
}
