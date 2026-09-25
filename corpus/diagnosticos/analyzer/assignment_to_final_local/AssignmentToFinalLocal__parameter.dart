// %before-language-feature: primary-constructors
f(final x) {
  x = 1;
//^
// [diag.assignmentToFinalLocal] The final variable 'x' can only be set once.
}
