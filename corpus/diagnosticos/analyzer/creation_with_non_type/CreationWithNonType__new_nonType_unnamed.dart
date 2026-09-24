void NonType() {}
f() {
  new NonType();
//    ^^^^^^^
// [diag.newWithNonType] The name 'NonType' isn't a class.
}
