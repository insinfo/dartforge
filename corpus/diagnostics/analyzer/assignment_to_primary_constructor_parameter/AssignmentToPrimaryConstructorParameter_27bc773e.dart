class A(int x) {
  Map<int?, int> y = {null: x} = {null: 2};
//                          ^
// [diag.assignmentToPrimaryConstructorParameter] A primary constructor parameter can't be assigned to in an initializer.
}
