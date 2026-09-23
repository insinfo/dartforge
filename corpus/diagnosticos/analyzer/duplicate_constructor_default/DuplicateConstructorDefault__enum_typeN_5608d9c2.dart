enum E {
  v;
  const E.new();
  const E.new();
//      ^^^^^
// [diag.duplicateConstructorDefault] The unnamed constructor is already defined.
//        ^^^
// [diag.unusedElement] The declaration 'E.new' isn't referenced.
}
