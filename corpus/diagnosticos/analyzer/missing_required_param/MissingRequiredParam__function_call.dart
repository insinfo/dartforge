void f({required int a}) {}

main() {
  f.call();
//  ^^^^
// [diag.missingRequiredArgument] The named parameter 'a' is required, but there's no corresponding argument.
}
