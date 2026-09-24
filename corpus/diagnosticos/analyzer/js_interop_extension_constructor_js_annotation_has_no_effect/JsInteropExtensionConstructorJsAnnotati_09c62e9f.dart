import 'dart:js_interop';

extension type Foo(JSObject _) {
  @JS()
  external void m();

  @JS()
  external int get g;

  @JS()
  external set s(int v);
}
