import 'dart:js_interop';

extension type Foo(JSObject _) {
  @JS()
// ^^
// [diag.jsInteropExtensionConstructorJsAnnotationHasNoEffect] The '@JS' annotation on an extension type constructor has no effect and is disallowed.
  external Foo.bar();
}
