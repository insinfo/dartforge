class A(int x) {
  int y;
  this : y = x++;
//           ^
// [diag.assignmentToPrimaryConstructorParameter] A primary constructor parameter can't be assigned to in an initializer.
}
