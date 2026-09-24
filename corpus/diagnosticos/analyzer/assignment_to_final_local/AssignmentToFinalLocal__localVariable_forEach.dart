f() {
  final i;
  for (i in [1, 2, 3]) {
//     ^
// [diag.assignmentToFinalLocal] The final variable 'i' can only be set once.
    print(i);
  }
}
