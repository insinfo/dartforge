class A<T> {
  m(T t) {}
}
f(A<String> a) {
  a.m(1);
//    ^
// [diag.argumentTypeNotAssignable] The argument type 'int' can't be assigned to the parameter type 'String'.
}
