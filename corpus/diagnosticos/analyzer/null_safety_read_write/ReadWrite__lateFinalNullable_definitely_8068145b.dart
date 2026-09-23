void f() {
  // ignore:unused_local_variable
  late final int? x;
  x = 0;
  x = 1;
//^
// [diag.lateFinalLocalAlreadyAssigned] The late final local variable is already assigned.
}
