// %before-language-feature: wildcard-variables

typedef F<_, _> = Map;
//        ^
// [context 1] The first definition of this name.
//           ^
// [diag.duplicateDefinition][context 1] The name '_' is already defined.
