void f() {
  final x;
  x; // 0
//^
// [diag.readPotentiallyUnassignedFinal] The final variable 'x' can't be read because it's potentially unassigned at this point.
  x();
//^
// [diag.readPotentiallyUnassignedFinal] The final variable 'x' can't be read because it's potentially unassigned at this point.
}
