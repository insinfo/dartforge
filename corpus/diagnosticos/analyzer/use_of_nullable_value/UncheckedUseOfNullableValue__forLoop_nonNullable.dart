m() {
  List x = [];
  for (var y in x) {}
//         ^
// [diag.unusedLocalVariable] The value of the local variable 'y' isn't used.
}
