// Convertido de tests/conformance/modules/cascades20 (módulo antigo do corpus de conformidade).
import 'values.dart';

void main() {
  var c = make()..add(effect(1))..add(effect(2));
  print(c.value);
  print(c == (c..add(3)));
  print(c.value);
  var missing = absent()?..add(effect(99))..add(effect(100));
  print(missing == null);
  Counter? present = c;
  var same = present?..add(effect(4));
  print(same == c);
  print(c.value);
  var h = Holder()..child = (make()..add(effect(5)))..child.add(6);
  print(h.child.value);
  c..hidden();
  print(c.value);
  List<int> values = []..add(1)..add(2)..[0] = 7;
  print(values);
  var r = (c, name: 'record')..$1.add(1)..name;
  print(r.$1.value);
  var callbacks = <int Function(int)>[]..add((x) => other(x));
  print(callbacks[0](10));
}
