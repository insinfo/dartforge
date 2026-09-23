void f(List<(int,)> x) {
  for (var (a,) in x) {}
//          ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
