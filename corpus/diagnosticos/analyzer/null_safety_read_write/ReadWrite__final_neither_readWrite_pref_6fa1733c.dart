void f(bool b) {
  // ignore:unused_local_variable
  final x;
  if (b) x = 0;
  ++x; // 0
//  ^
// [diag.readPotentiallyUnassignedFinal] The final variable 'x' can't be read because it's potentially unassigned at this point.
// [diag.assignmentToFinalLocal] The final variable 'x' can only be set once.
}
