void f() {
  late int v;
//         ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
  v += 1;
//^
// [diag.definitelyUnassignedLateLocalVariable] The late local variable 'v' is definitely unassigned at this point.
}
