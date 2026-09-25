f() {
  final x = 0;
  for (x in <int>[1, 2]) {
//     ^
// [diag.assignmentToFinalLocal] The final variable 'x' can only be set once.
    print(x);
  }
}
