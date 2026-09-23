f() {
  late final int i;
  for (i in [1, 2, 3]) {
//     ^
// [diag.lateFinalLocalAlreadyAssigned] The late final local variable is already assigned.
    print(i);
  }
}
