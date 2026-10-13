class A {
  const A.named({required int a});
}

@A.named()
// ^^^^^
// [diag.missingRequiredArgument] The named parameter 'a' is required, but there's no corresponding argument.
void f() {}
