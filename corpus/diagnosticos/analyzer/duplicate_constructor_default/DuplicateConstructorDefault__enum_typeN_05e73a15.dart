enum E {
  v;
  const E();
  factory () => v;
//^^^^^^^
// [diag.duplicateConstructorDefault] The unnamed constructor is already defined.
}
