import 'dart:collection';

final events = <int>[];
void record(int id) {
  events.add(id);
  if (id == 13) throw StateError('constructor');
}

class Item {
  final int id;
  Item(this.id) { record(id); }
}
class Child extends Item { Child(super.id); }
class Custom extends ListBase<Item> {
  final List<Item> values = [];
  int get length => values.length;
  set length(int value) { values.length = value; }
  Item operator [](int i) => values[i];
  void operator []=(int i, Item value) { values[i] = value; }
  void add(Item value) { events.add(-100); values.add(value); }
}

List<Item> fresh(int n) {
  final list = <Item>[];
  for (var i = 0; i < n; i++) { list.add(Item(i)); }
  return list;
}
List<Item> subtype(int n) {
  final list = <Item>[];
  for (var i = 0; i < n; i++) { list.add(Child(i)); }
  return list;
}
List<Item> unknownFactory(int n) {
  final list = <Item>[];
  for (var i = 0; i < n; i++) { list.add(make(i)); }
  return list;
}
Item make(int n) => Item(n);
List<Item>? published;
List<Item> escaped(int n) {
  final list = <Item>[];
  published = list;
  for (var i = 0; i < n; i++) { list.add(Item(i)); }
  return list;
}
List<Item> aliased(int n) {
  final list = <Item>[];
  final alias = list;
  for (var i = 0; i < n; i++) { alias.add(Item(i)); }
  return list;
}
List<Item> captured(int n) {
  final list = <Item>[];
  void append(int id) { list.add(Item(id)); }
  for (var i = 0; i < n; i++) { append(i); }
  return list;
}
List<Item> rebound(int n) {
  var list = <Item>[];
  list = <Item>[Item(7)];
  for (var i = 0; i < n; i++) { list.add(Item(i)); }
  return list;
}
List<Item> covariant(int n) {
  final List<Item> list = <Child>[];
  for (var i = 0; i < n; i++) { list.add(Item(i)); }
  return list;
}
List<Item> fixed(int n) {
  final list = List<Item>.empty(growable: false);
  for (var i = 0; i < n; i++) { list.add(Item(i)); }
  return list;
}
List<Item> custom(int n) {
  final list = Custom();
  for (var i = 0; i < n; i++) { list.add(Item(i)); }
  return list;
}
List<String> strings() => <String>[];
void main() {
  for (final build in [fresh, subtype, unknownFactory, escaped, aliased, captured, rebound, covariant, fixed, custom]) {
    events.clear();
    try {
      final values = build(3);
      print('${values.map((v) => v.id).toList()}:${values is List<Item>}:$events');
    } catch (error) { print('${error is TypeError}:${error is UnsupportedError}:$events'); }
  }
  events.clear();
  print('${fresh(0).length}:$events');
  try { fresh(14); } catch (error) { print('${error is StateError}:$events'); }
  final List<Object> view = fresh(1);
  try { view.add('wrong'); } catch (error) { print(error is TypeError); }
  print(strings().length);
}
