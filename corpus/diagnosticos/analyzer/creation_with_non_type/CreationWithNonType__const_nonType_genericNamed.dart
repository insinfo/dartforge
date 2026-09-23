void NonType<T>() {}
f() {
  const NonType<int>.named();
//      ^^^^^^^
// [diag.constWithNonType] The name 'NonType' isn't a class.
}
