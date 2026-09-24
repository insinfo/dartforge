class A {
  int x;
  final Object y;
  A(this.x) : y = (() {
    x = 0;
//  ^
// [diag.assignmentToFinalLocal] The final variable 'x' can only be set once.
  });
}
