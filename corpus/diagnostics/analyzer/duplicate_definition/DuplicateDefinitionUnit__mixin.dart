mixin A {}
//    ^
// [context 1] The first definition of this name.
mixin B {}
mixin A {}
//    ^
// [diag.duplicateDefinition][context 1] The name 'A' is already defined.
