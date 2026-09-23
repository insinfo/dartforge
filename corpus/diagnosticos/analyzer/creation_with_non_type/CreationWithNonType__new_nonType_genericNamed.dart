void NonType<T>() {}
f() {
  new NonType<int>.named();
//    ^^^^^^^
// [diag.newWithNonType] The name 'NonType' isn't a class.
}
