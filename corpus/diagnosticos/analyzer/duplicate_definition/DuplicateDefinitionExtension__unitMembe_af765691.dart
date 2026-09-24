class A {}
extension E on A {}
//        ^
// [context 1] The first definition of this name.
extension E on A {}
//        ^
// [diag.duplicateDefinition][context 1] The name 'E' is already defined.
