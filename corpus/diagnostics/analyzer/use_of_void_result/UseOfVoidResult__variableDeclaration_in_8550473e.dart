int f() => 1;
g() {
  var a = f();
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
