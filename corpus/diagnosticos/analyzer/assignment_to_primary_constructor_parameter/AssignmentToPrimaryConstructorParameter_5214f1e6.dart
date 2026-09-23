class A(int x) {
  int y = (x) = 0;
//         ^
// [diag.assignmentToPrimaryConstructorParameter] A primary constructor parameter can't be assigned to in an initializer.
}
