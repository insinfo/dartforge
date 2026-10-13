void foo<T>() {
  new T();
//    ^
// [diag.newWithNonType] The name 'T' isn't a class.
}
