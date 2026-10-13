class A(int? x) {
  int y = (x!) = 2;
//         ^
// [diag.assignmentToPrimaryConstructorParameter] A primary constructor parameter can't be assigned to in an initializer.
//          ^
// [diag.unnecessaryNullAssertPattern] The null-assert pattern will have no effect because the matched type isn't nullable.
}
