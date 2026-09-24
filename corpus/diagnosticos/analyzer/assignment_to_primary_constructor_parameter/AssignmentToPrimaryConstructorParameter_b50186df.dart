class A(int x) {
  Object y = int(sign: x) = 2;
//                     ^
// [diag.assignmentToPrimaryConstructorParameter] A primary constructor parameter can't be assigned to in an initializer.
}
