import 'dart:js_interop';

extension type A(B _) {
//             ^
// [diag.extensionTypeRepresentationDependsOnItself] The extension type representation can't depend on itself.
  @JS()
  external A.bar();
}

extension type B(A _) {}
//             ^
// [diag.extensionTypeRepresentationDependsOnItself] The extension type representation can't depend on itself.
