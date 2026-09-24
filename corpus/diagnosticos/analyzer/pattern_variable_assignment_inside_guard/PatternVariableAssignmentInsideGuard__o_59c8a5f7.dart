void f(int x) {
  // ignore:unused_local_variable
  var b = 0;
  if (x case var a when (b = 1) > 0) {}
//               ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
