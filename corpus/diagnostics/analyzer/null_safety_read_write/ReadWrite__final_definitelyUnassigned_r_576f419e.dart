void f() {
  // ignore:unused_local_variable
  final x;
  x++;
//^
// [diag.readPotentiallyUnassignedFinal] The final variable 'x' can't be read because it's potentially unassigned at this point.
}
