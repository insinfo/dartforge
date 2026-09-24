f() {
  const x = 0;
  for (x in <int>[1, 2]) {
//     ^
// [diag.assignmentToConst] Constant variables can't be assigned a value after initialization.
    print(x);
  }
}
