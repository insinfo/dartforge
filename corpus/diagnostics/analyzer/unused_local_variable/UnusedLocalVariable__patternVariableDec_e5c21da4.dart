void f() {
  var (a, b) = () {
//     ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
//        ^
// [diag.unusedLocalVariable] The value of the local variable 'b' isn't used.
    var (c, d) = (0, 1);
    return (c, d);
  }();
}
