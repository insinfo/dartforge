// diverge-ddc: `dart:js_interop` only exists on the web; the VM does not even compile the program.
// `external` members of an `extension` on an interop type (sanitize_dom's
// `extension SafeElement on Element { @JS('innerHTML') external String get
// innerHtml; }`), in a library without `@JS()`: the CFE lowers them to a
// direct read, call and write on the receiver, and the extension has no
// `Ext|get#m` function for them. Also `isA<T>()` with the types of
// `dart:js_interop` itself (popper's `isA<JSObject>()`), including through
// the implicit `this` of an extension.

import 'dart:js_interop';

@JS('JSON.parse')
external JSAny? parse(String text);

@JS('Array')
extension type JsList._(JSObject _) implements JSObject {
  external int get length;
}

@JS('Object')
extension type Box._(JSObject _) implements JSObject {}

@JS('Uint8Array')
extension type Bytes._(JSObject _) implements JSObject {
  external Bytes(int n);
}

extension OnList on JsList {
  @JS('indexOf')
  external int position(int x);
  external JSAny? at(int i);
}

extension Safe on Box {
  @JS('innerHTML')
  external String get innerHtml;

  set innerHtml(String v) {
    label = 'set:$v';
  }

  external String get label;
  external set label(String v);

  bool get isList => isA<JsList>();
  bool isListMethod() => this.isA<JsList>();
  String describe() => '$innerHtml/${isA<JsList>()}';
}

void kinds(JSAny? v) {
  print([
    v.isA<JSAny>(),
    v.isA<JSAny?>(),
    v.isA<JSObject>(),
    v.isA<JSObject?>(),
    v.isA<JSArray>(),
    v.isA<JSArray<JSNumber>>(),
    v.isA<JSFunction>(),
    v.isA<JSBoxedDartObject>(),
    v.isA<JSArrayBuffer>(),
    v.isA<JSDataView>(),
    v.isA<JSTypedArray>(),
    v.isA<JSUint8Array>(),
    v.isA<JSFloat64Array>(),
    v.isA<JSPromise>(),
    v.isA<JSNumber>(),
    v.isA<JSString>(),
  ].map((b) => b ? 1 : 0).join());
}

void main() {
  final c = parse('{"innerHTML":"<b>x</b>","label":"r"}') as Box;
  print(c.label);
  print(c.innerHtml);
  print(c.isList);
  print(c.isListMethod());
  print(c.describe());
  c.innerHtml = 'y';
  print(c.label);
  c.label += '!';
  print(c.label);
  final l = parse('[1]') as Box;
  print(l.isList);
  print((l as JsList).position(1));
  print((l as JsList).position(7));
  print((l as JsList).at(0).isA<JSNumber>());
  kinds(null);
  kinds(parse('{}'));
  kinds(parse('[1]'));
  kinds(parse('2'));
  kinds(parse('"s"'));
  kinds(Bytes(2));
  kinds(kinds.toJS);
}
