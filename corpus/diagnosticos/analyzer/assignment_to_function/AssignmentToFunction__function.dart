f() {}
main() {
  f = null;
//^
// [diag.assignmentToFunction] Functions can't be assigned a value.
}
