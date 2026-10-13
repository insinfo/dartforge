void f({required int a}) {}

main() {
  f();
//^
// [diag.missingRequiredArgument] The named parameter 'a' is required, but there's no corresponding argument.
}
