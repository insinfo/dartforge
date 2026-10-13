var A = 0;
f(p) {
  if (p is A) {
//         ^
// [diag.typeTestWithNonType] The name 'A' isn't a type and can't be used in an 'is' expression.
  }
}
