enum E {
  v(a: 0, a: 1);
//        ^
// [diag.duplicateNamedArgument] The argument for the named parameter 'a' was already specified.
  const E({required int a});
}
