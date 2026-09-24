class C.foo() {
  C.foo() : this.foo();
//^^^^^
// [diag.duplicateConstructorName] The constructor with name 'foo' is already defined.
}
