void nonPrefix() {}
f() {
  new NonType<int>();
//    ^^^^^^^
// [diag.newWithNonType] The name 'NonType' isn't a class.
}
