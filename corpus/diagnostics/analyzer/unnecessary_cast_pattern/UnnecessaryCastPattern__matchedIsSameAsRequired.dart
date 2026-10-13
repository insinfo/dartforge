void f(int x) {
  if (x case var z as int) {}
//               ^
// [diag.unusedLocalVariable] The value of the local variable 'z' isn't used.
//                 ^^
// [diag.unnecessaryCastPattern] Unnecessary cast pattern.
}
