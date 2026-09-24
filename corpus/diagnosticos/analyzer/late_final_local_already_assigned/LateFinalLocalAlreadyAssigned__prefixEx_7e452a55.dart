main() {
  late final int v = 0;
  ++v;
//  ^
// [diag.lateFinalLocalAlreadyAssigned] The late final local variable is already assigned.
  v;
}
