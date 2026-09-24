class A {
  const A(String p);
}
main() {
  const A(42);
//        ^^
// [diag.argumentTypeNotAssignable] The argument type 'int' can't be assigned to the parameter type 'String'.
// [diag.constConstructorParamTypeMismatch] A value of type 'int' can't be assigned to a parameter of type 'String' in a const constructor.
}
