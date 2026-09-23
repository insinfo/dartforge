typedef void F();
main() {
  F = null;
//^
// [diag.assignmentToType] Types can't be assigned a value.
}
