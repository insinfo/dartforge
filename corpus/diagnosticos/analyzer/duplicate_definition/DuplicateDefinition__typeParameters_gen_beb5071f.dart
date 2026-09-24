// %before-language-feature: wildcard-variables

typedef F<_, _> = void Function();
//        ^
// [context 1] The first definition of this name.
//           ^
// [diag.duplicateDefinition][context 1] The name '_' is already defined.
