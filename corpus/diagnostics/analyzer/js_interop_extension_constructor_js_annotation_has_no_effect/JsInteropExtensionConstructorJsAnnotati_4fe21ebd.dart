import 'dart:js_interop';

extension type Foo(JSObject _) {
  @JS()
  Foo.bar(JSObject o) : this(o);
}
