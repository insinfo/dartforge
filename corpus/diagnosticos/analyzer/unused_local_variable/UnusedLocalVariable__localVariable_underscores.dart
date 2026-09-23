f() {
  var __ = 0;
//    ^^
// [diag.unusedLocalVariable] The value of the local variable '__' isn't used.
  var ___ = 0;
//    ^^^
// [diag.unusedLocalVariable] The value of the local variable '___' isn't used.
}
