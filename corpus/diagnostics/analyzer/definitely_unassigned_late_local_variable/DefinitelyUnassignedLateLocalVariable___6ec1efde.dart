void f() {
  late int v;
  v++;
//^
// [diag.definitelyUnassignedLateLocalVariable] The late local variable 'v' is definitely unassigned at this point.
  v;
}
