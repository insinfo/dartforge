class C {
  new foo();
  new foo();
//^^^^^^^
// [diag.duplicateConstructorName] The constructor with name 'foo' is already defined.
}
