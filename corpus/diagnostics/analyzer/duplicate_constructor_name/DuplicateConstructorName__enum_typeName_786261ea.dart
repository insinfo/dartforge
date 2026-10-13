enum E {
  v.foo();
  const E.foo();
  const E.foo();
//      ^^^^^
// [diag.duplicateConstructorName] The constructor with name 'foo' is already defined.
//        ^^^
// [diag.unusedElement] The declaration 'E.foo' isn't referenced.
}
