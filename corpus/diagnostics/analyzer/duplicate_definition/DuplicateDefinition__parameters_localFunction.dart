main() {
  f(int a, double a) {
//^
// [diag.unusedElement] The declaration 'f' isn't referenced.
//      ^
// [context 1] The first definition of this name.
//                ^
// [diag.duplicateDefinition][context 1] The name 'a' is already defined.
  };
}
