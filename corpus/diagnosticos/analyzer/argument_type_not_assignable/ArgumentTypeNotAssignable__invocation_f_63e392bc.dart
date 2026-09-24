class A<K, V> {
  m(f(K k), V v) {
    f(v);
//    ^
// [diag.argumentTypeNotAssignable] The argument type 'V' can't be assigned to the parameter type 'K'.
  }
}
