extension type A(int it) {}
//             ^
// [context 1] The first definition of this name.
extension type B(int it) {}
extension type A(int it) {}
//             ^
// [diag.duplicateDefinition][context 1] The name 'A' is already defined.
