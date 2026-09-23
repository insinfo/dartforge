class A(int x) {
  (int, {bool name}) y = (x, name: _) = (2, name: true);
//                        ^
// [diag.assignmentToPrimaryConstructorParameter] A primary constructor parameter can't be assigned to in an initializer.
}
