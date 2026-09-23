// %before-language-feature: wildcard-variables

typedef void F<_, _>();
//             ^
// [context 1] The first definition of this name.
//                ^
// [diag.duplicateDefinition][context 1] The name '_' is already defined.
