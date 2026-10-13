f() {
  var x;
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'x' isn't used.
  var y;
  x = y;
}
