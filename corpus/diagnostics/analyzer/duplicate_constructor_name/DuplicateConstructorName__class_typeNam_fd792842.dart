class C {
  factory C.foo() => throw 0;
  factory foo() => throw 0;
//^^^^^^^^^^^
// [diag.duplicateConstructorName] The constructor with name 'foo' is already defined.
}
