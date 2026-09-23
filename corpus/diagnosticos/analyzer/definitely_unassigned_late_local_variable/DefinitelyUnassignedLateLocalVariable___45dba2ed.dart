void f() {
  late int v;
  v += 1;
//^
// [diag.definitelyUnassignedLateLocalVariable] The late local variable 'v' is definitely unassigned at this point.
  v;
}
