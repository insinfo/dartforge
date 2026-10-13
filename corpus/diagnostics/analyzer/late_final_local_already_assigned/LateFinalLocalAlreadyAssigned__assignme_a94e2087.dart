main() {
  late final int v = 0;
  v = 1;
//^
// [diag.lateFinalLocalAlreadyAssigned] The late final local variable is already assigned.
  v;
}
