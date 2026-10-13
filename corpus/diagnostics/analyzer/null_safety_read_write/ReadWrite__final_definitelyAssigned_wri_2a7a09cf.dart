// %before-language-feature: primary-constructors
void f(final x) {
  for (x in [0, 1, 2]) {
//     ^
// [diag.assignmentToFinalLocal] The final variable 'x' can only be set once.
    x;
  }
}
