// %before-language-feature: wildcard-variables

class A {
  int? _;
//     ^
// [diag.unusedField] The value of the field '_' isn't used.
  A(int _, this._);
//      ^
// [context 1] The first definition of this name.
//              ^
// [diag.duplicateDefinition][context 1] The name '_' is already defined.
}
