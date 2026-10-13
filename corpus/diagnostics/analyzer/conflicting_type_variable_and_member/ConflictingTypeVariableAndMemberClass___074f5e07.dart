// %before-language-feature: wildcard-variables

class A<_> {
//      ^
// [diag.conflictingTypeVariableAndMemberClass] '_' can't be used to name both a type parameter and a member in this class.
  _() {}
//^
// [diag.unusedElement] The declaration '_' isn't referenced.
}
