enum E {
  v;
  const E();
  const E();
//      ^
// [diag.duplicateConstructorDefault] The unnamed constructor is already defined.
}
