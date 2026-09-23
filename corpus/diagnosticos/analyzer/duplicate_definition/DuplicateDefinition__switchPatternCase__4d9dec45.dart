// %before-language-feature: wildcard-variables

void f() {
  switch (0) {
    case 0:
      var _;
//        ^
// [context 1] The first definition of this name.
      var _;
//        ^
// [diag.duplicateDefinition][context 1] The name '_' is already defined.
  }
}
