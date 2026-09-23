enum E {
  v1, v2();
  const E([int? a]);
//              ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
