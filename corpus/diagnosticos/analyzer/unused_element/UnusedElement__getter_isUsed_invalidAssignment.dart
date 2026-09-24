int get _g => 0;

void f() {
  _g = 1;
//^^
// [diag.assignmentToFinal] '_g' can't be used as a setter because it's final.
}
