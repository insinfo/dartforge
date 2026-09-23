void f(dynamic x) {
  var (int a) = x;
//         ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
