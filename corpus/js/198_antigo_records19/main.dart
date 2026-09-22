// Convertido de tests/conformance/modules/records19 (módulo antigo do corpus de conformidade).
import 'values.dart';

void main() {
  final (name, age) = user();
  print(name);
  print(age);
  var (name: otherName, age: otherAge) = namedUser();
  otherAge += 1;
  print(otherName);
  print(otherAge);
  var value = pair<int>(7);
  print(value.$1);
  print(value.other);
  print(accepts<(int, {int other})>(value));
  print(accepts<(Object, {Object other})>(value));
  print(accepts<(String, {String other})>(value));
  Object erased = (identity<Object>(1),);
  print(accepts<(int,)>(erased));
  print((1, a: 'a', b: true) == (b: true, 1, a: 'a'));
  print(((1, 2),) == ((1, 2),));
  print((<int>[1],) == (<int>[1],));
  print(() == ());
  var effects = (effect(1), second: effect(2), effect(3));
  print(effects.$2);
  print(effects.second);
  var (_, kept) = (effect(4), effect(5));
  print(kept);
  var names = (z: 3, a: 'first');
  final (:a, :z) = names;
  print(a);
  print(z);
  var lists = <(int, String)>[(1, 'one')];
  print(lists[0].$2);
  (int, String)? nullable = null;
  print(accepts<(int, String)?>(nullable));
  nullable = (2, 'two');
  if (nullable != null) { print(nullable.$1); }
}
