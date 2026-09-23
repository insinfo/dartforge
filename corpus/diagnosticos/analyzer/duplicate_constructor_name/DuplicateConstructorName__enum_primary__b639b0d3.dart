enum E.foo() {
  v.foo();
  factory E.foo() => v;
//        ^^^^^
// [diag.duplicateConstructorName] The constructor with name 'foo' is already defined.
}
