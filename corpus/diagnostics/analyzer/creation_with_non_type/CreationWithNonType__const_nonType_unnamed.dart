void NonType() {}
f() {
  const NonType();
//      ^^^^^^^
// [diag.constWithNonType] The name 'NonType' isn't a class.
}
