void f() {
  var (a) = 1;
//     ^
// [context 1] The first definition of this name.
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
  var a = 0;
//    ^
// [diag.duplicateDefinition][context 1] The name 'a' is already defined.
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
