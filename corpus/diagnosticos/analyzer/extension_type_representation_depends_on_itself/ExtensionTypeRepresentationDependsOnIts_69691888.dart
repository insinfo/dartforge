extension type A(List<B> it) {}
//             ^
// [diag.extensionTypeRepresentationDependsOnItself] The extension type representation can't depend on itself.

extension type B(List<A> it) {}
//             ^
// [diag.extensionTypeRepresentationDependsOnItself] The extension type representation can't depend on itself.
