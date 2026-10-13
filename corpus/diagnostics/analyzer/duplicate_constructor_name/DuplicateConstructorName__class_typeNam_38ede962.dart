class C {
  C.foo();
  new foo();
//^^^^^^^
// [diag.duplicateConstructorName] The constructor with name 'foo' is already defined.
}
