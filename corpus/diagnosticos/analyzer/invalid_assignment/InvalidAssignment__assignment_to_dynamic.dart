f() {
  var g;
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'g' isn't used.
  g = () => 0;
}
