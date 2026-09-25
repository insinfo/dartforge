f(String p) {}
main() {
  f(42);
//  ^^
// [diag.argumentTypeNotAssignable] The argument type 'int' can't be assigned to the parameter type 'String'.
}
