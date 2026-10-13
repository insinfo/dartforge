main() {
  f(int value) {}
  f = 0;
//^
// [diag.assignmentToFunction] Functions can't be assigned a value.
}
