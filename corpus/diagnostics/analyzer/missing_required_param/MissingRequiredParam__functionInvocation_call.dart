void Function({required int a}) f() => throw '';
g() {
  f().call();
//    ^^^^
// [diag.missingRequiredArgument] The named parameter 'a' is required, but there's no corresponding argument.
}
