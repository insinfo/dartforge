class A(var int _, var int _);
//              ^
// [context 1] The first definition of this name.
// [diag.unusedFieldFromPrimaryConstructor] The value of the field '_' isn't used.
//                         ^
// [diag.duplicateDefinition][context 1] The name '_' is already defined.
// [diag.unusedFieldFromPrimaryConstructor] The value of the field '_' isn't used.
