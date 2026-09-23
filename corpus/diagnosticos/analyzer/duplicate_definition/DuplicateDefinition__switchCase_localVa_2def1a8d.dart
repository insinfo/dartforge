// %before-language-feature: patterns
void f() {
  switch (0) {
    case 0:
      var a;
//        ^
// [context 1] The first definition of this name.
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
      var a;
//        ^
// [diag.duplicateDefinition][context 1] The name 'a' is already defined.
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
  }
}
