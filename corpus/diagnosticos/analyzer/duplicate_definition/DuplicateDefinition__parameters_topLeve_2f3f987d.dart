// %before-language-feature: wildcard-variables

f(int _, double _) {}
//    ^
// [context 1] The first definition of this name.
//              ^
// [diag.duplicateDefinition][context 1] The name '_' is already defined.
