void f() {
  for (var (a,) = (0,);;) {}
//          ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
