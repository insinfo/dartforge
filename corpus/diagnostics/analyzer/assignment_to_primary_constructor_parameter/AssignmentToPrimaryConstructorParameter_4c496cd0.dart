class A(int x) {
  List<int> y = [x] = [2];
//               ^
// [diag.assignmentToPrimaryConstructorParameter] A primary constructor parameter can't be assigned to in an initializer.
}
