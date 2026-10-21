// dart:js_interop values and conversions: globalContext, js_interop_unsafe
// properties and methods, num/bool/String crossings (Latin-1 and UTF-16 texts),
// JSValue equality, hashCode and toString, and the operators of JSAny.
import 'dart:js_interop';
import 'dart:js_interop_unsafe';

void main() {
  final m = globalContext['Math'] as JSObject;
  print(m.callMethod<JSNumber>('max'.toJS, 3.toJS, 7.toJS).toDartInt);
  final o = JSObject();
  o['x'] = 'hi é ☃'.toJS;
  print((o['x'] as JSString).toDart);
  print(o == o);
  print(o);
  print(o.hashCode);
  print(globalContext['undefinedThing']);
  print(o.has('x'));
  print(o.typeofEquals('object'));
  print(1.5.toJS.add(2.toJS));
}
