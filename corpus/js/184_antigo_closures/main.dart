// Convertido de tests/conformance/modules/closures (módulo antigo do corpus de conformidade).
import 'helper.dart';
import 'dart:core';
void main() {
  var first = makeCounter(3);
  var second = makeCounter(8);
  print(first());
  print(second());
  print(first());
  print(first == second);
  print(selected() == left);
  print(left == right);
  print(invoke(selected(), 4));
  var values = mapValues(<int>[2, 4], (value) => value + 5);
  print(values[0]);
  print(values[1]);
}
