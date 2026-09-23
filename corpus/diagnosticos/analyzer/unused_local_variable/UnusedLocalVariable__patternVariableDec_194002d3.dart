void f() {
  var (a, [b, _]) = (0, []);
//     ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
//         ^
// [diag.unusedLocalVariable] The value of the local variable 'b' isn't used.
}
