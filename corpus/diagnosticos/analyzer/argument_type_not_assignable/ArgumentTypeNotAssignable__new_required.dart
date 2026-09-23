class A {
  A(String p) {}
}
main() {
  new A(42);
//      ^^
// [diag.argumentTypeNotAssignable] The argument type 'int' can't be assigned to the parameter type 'String'.
}