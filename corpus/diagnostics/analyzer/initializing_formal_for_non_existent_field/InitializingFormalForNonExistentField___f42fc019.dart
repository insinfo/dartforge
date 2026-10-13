enum E {
  v;
  const E([this.x]);
//         ^^^^^^
// [diag.initializingFormalForNonExistentField] 'x' isn't a field in the enclosing class.
//              ^
// [diag.unusedElementParameter] A value for optional parameter 'x' isn't ever given.
}
