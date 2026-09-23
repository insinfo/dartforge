f() {
  int x;
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'x' isn't used.
  x = '0';
//    ^^^
// [diag.invalidAssignment] A value of type 'String' can't be assigned to a variable of type 'int'.
}
