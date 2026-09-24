class A(int x) {
  var f = () {
    x = 0;
//  ^
// [diag.assignmentToPrimaryConstructorParameter] A primary constructor parameter can't be assigned to in an initializer.
  };
}
