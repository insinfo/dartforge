void Function({required int a}) f() => throw '';
g() {
  f()();
//^^^^^
// [diag.missingRequiredArgument] The named parameter 'a' is required, but there's no corresponding argument.
}
