extension type A(B it) {}
//             ^
// [diag.extensionTypeRepresentationDependsOnItself] The extension type representation can't depend on itself.

extension type B(A it) {}
//             ^
// [diag.extensionTypeRepresentationDependsOnItself] The extension type representation can't depend on itself.
