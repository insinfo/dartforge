// createJSInteropWrapper with @JSExport: a whole class exported (methods,
// fields, getters and setters, inherited members and overrides), only the
// annotated members of a class, renamed exports, and the wrapper used from
// JavaScript through an interop extension type.
import 'dart:js_interop';
import 'dart:js_interop_unsafe';

@JS('eval')
external JSAny? jsEval(String code);

@JS('Object.keys')
external JSArray<JSString> keys(JSObject o);

@JSExport()
class Base {
  int count = 1;
  String describe() => 'base $count';
  String get kind => 'base';
}

@JSExport()
class Counter extends Base {
  final String name;
  Counter(this.name);

  void increment([int by = 1]) {
    count += by;
  }

  @override
  String describe() => 'counter $name=$count';

  int get doubled => count * 2;
  set doubled(int v) {
    count = v ~/ 2;
  }

  static int ignoredStatic() => 0;
}

class Partial {
  @JSExport()
  int visible = 3;
  int hidden = 4;

  @JSExport('renamed')
  String original(String s) => '[$s]';
}

extension type CounterJS(JSObject _) implements JSObject {
  external int count;
  external String get name;
  external String get kind;
  external int doubled;
  external void increment([int by]);
  external String describe();
}

void main() {
  final c = Counter('a');
  final js = createJSInteropWrapper(c) as CounterJS;
  print(js.count);
  js.increment();
  js.increment(5);
  print(c.count);
  print(js.describe());
  print(js.name);
  print(js.kind);
  js.doubled = 40;
  print(c.count);
  print(js.doubled);
  js.count = 7;
  print(c.describe());
  print(keys(js).toDart.map((k) => k.toDart).toList()..sort());
  print(js.has('ignoredStatic'));

  final p = Partial();
  final pj = createJSInteropWrapper<Partial>(p);
  print(pj.has('visible'));
  print(pj.has('hidden'));
  print(pj.has('original'));
  print(pj['visible']);
  pj['visible'] = 9.toJS;
  print(p.visible);
  print(pj.callMethod<JSString>('renamed'.toJS, 'x'.toJS).toDart);

  // From JavaScript code.
  final f = jsEval('(o) => { o.increment(2); return o.describe() + "/" + o.doubled; }')
      as JSFunction;
  print((f.callAsFunction(null, js) as JSString).toDart);
}
