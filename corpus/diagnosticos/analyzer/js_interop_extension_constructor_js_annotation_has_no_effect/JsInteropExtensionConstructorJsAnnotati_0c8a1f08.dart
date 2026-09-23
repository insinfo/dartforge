import 'dart:js_interop';

@JS()
@staticInterop
class Foo {
  @JS()
  external factory Foo.bar();
}
