enum E {
  v(0);
//  ^
// [diag.extraPositionalArgumentsCouldBeNamed] Too many positional arguments: 0 expected, but 1 found.
  const E({int? a});
//              ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
