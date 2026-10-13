void NonType<T>() {}
f() {
  const NonType<int>();
//      ^^^^^^^
// [diag.constWithNonType] The name 'NonType' isn't a class.
}
