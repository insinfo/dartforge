void g(void Function(bool) fun) {}

void f() {
  g((value) => {if (value) print('')});
//             ^^^^^^^^^^^^^^^^^^^^^^
// [diag.unnecessarySetLiteral] Braces unnecessarily wrap this expression in a set literal.
}
