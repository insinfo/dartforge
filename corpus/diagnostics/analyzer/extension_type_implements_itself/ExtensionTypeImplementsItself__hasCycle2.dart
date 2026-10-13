extension type A(int it) implements B {}
//             ^
// [diag.extensionTypeImplementsItself] The extension type can't implement itself.
extension type B(int it) implements A {}
//             ^
// [diag.extensionTypeImplementsItself] The extension type can't implement itself.
