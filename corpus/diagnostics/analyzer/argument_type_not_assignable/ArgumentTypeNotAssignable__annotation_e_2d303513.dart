extension type const A(String _) {}

@A(0)
// ^
// [diag.argumentTypeNotAssignable] The argument type 'int' can't be assigned to the parameter type 'String'.
void f() {}
