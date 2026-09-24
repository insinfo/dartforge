class A {
  void m({required int a}) {}
}
f() {
  new A().m();
//        ^
// [diag.missingRequiredArgument] The named parameter 'a' is required, but there's no corresponding argument.
}
