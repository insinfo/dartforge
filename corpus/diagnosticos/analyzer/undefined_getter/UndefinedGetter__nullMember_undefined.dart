m() {
  Null _null;
  _null.foo;
//      ^^^
// [diag.invalidUseOfNullValue] An expression whose value is always 'null' can't be dereferenced.
}
