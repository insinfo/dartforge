// diverge-ddc: `dart:js_interop` only exists on the web; the VM does not even compile the program.
// Non-`external` static members of an interop extension type (package:web's
// `extension type Node._(JSObject _) { static const int ELEMENT_NODE = 1; }`,
// read by sanitize_dom in a `switch`): the constants fold to their value, and
// the other statics live on the type's support object, as in a class.
@JS()
library interop_extension_type_statics;

import 'dart:js_interop';

@JS('JSON.parse')
external JSAny? parse(String text);

@JS('Object')
extension type Box._(JSObject _) implements JSObject {
  static const int one = 1;
  static const int two = one + 1;
  static const String label = 'box';
  static const List<int> list = [one, two];
  static int counter = 0;
  static final int computed = twice(21);
  static late int lateValue = counter + 100;
  static int twice(int x) => x * 2;
  static int get getter => counter * 10;
  static set setter(int v) => counter = v;
  external int get kind;
}

String describe(Box b) {
  switch (b.kind) {
    case Box.one:
      return 'one';
    case Box.two:
      return 'two';
    default:
      return 'other';
  }
}

void main() {
  print(Box.one);
  print(Box.two);
  print(Box.label);
  print(Box.list);
  print(Box.counter);
  Box.counter++;
  Box.counter += 2;
  print(Box.counter);
  print(Box.computed);
  print(Box.lateValue);
  print(Box.twice(4));
  print(Box.getter);
  Box.setter = 7;
  print(Box.counter);
  final f = Box.twice;
  print(f(5));
  for (final s in ['{"kind":1}', '{"kind":2}', '{"kind":3}']) {
    print(describe(parse(s) as Box));
  }
}
