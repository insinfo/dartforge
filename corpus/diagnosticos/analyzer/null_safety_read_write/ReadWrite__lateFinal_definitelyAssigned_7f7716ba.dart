void f() {
  // ignore:unused_local_variable
  late final x;
  x = 0;
  ++x; // 0
//  ^
// [diag.lateFinalLocalAlreadyAssigned] The late final local variable is already assigned.
}
