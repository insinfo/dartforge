class A {
  const A({required int a});
}

@A()
// [diag.missingRequiredArgument][column 2][length 1] The named parameter 'a' is required, but there's no corresponding argument.
void f() {}
