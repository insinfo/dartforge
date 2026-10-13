class A<T> {
  void m([A<T> Function() fn = A.new]) {}
//                             ^^^^^
// [diag.invalidAssignment] A value of type 'A<dynamic> Function()' can't be assigned to a variable of type 'A<T> Function()'.
}
