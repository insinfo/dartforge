void f() {
  var [a, a] = [0, 1];
//     ^
// [context 1] The first definition of this name.
//        ^
// [diag.duplicateVariablePattern][context 1] The variable 'a' is already defined in this pattern.
  a;
}
