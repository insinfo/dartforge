// %before-language-feature: primary-constructors
void f(final x) {
  ++x;
//  ^
// [diag.assignmentToFinalLocal] The final variable 'x' can only be set once.
}
