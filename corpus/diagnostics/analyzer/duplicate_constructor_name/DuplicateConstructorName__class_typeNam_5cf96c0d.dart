class C {
  C.foo();
  C.foo();
//^^^^^
// [diag.duplicateConstructorName] The constructor with name 'foo' is already defined.
}
