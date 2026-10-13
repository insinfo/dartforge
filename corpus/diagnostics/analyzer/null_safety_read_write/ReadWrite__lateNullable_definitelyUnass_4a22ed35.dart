void f() {
  late int? x;
  x; // 0
//^
// [diag.definitelyUnassignedLateLocalVariable] The late local variable 'x' is definitely unassigned at this point.
}
