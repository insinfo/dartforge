typedef F = List<int>;

void f() {
  F = null;
//^
// [diag.assignmentToType] Types can't be assigned a value.
}
