void f(int x) {
  if (x case var a!) {}
//               ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
//                ^
// [diag.unnecessaryNullAssertPattern] The null-assert pattern will have no effect because the matched type isn't nullable.
}
