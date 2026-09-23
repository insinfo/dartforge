extension A on int {}
//        ^
// [context 1] The first definition of this name.
extension B on int {}
extension A on int {}
//        ^
// [diag.duplicateDefinition][context 1] The name 'A' is already defined.
