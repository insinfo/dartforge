void f() {
  // ignore:unused_local_variable
  late final x;
  ++x; // 0
//  ^
// [diag.definitelyUnassignedLateLocalVariable] The late local variable 'x' is definitely unassigned at this point.
}
