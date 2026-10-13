enum E {
  v;
  const E();
  const new ();
//      ^^^
// [diag.duplicateConstructorDefault] The unnamed constructor is already defined.
}
