import 'dart:js_interop' as js;

extension type Foo(js.JSObject _) {
  @js.JS()
// ^^^^^
// [diag.jsInteropExtensionConstructorJsAnnotationHasNoEffect] The '@JS' annotation on an extension type constructor has no effect and is disallowed.
  external Foo.bar();
}
