enum E { A }
f(E e) {
  e[0];
// ^^^
// [diag.undefinedOperator] The operator '[]' isn't defined for the type 'E'.
}
