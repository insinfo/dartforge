class A<T> {
  A(T p) {}
}
main() {
  new A<String>(42);
//              ^^
// [diag.argumentTypeNotAssignable] The argument type 'int' can't be assigned to the parameter type 'String'.
}