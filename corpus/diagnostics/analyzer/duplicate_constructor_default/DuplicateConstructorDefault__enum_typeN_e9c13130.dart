enum E {
  v;
  const E.new();
  const E();
//      ^
// [diag.duplicateConstructorDefault] The unnamed constructor is already defined.
}
