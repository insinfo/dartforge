import 'dart:js_interop';

extension type Wrapper(JSObject _) {}

extension type Foo(Wrapper _) {
  @JS()
// ^^
// [diag.jsInteropExtensionConstructorJsAnnotationHasNoEffect] The '@JS' annotation on an extension type constructor has no effect and is disallowed.
  external Foo.bar();
}
