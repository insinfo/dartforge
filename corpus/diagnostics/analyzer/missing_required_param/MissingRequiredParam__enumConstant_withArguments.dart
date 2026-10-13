enum E {
  v();
//^
// [diag.missingRequiredArgument] The named parameter 'a' is required, but there's no corresponding argument.
  const E({required int a});
}
