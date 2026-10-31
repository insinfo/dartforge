// More collection operations: Set<int>, Map<String, int>, iteration of maps,
// List methods (where/map/toList, contains, indexOf, insert/removeLast,
// sublist, addAll, join), and LinkedHashMap removal.
import '../performance/common.dart';

int setInt(int n) {
  final s = <int>{};
  for (var i = 0; i < n; i++) {
    s.add(i * 13 % (n ~/ 2));
  }
  var c = 0;
  for (var i = 0; i < n; i++) {
    if (s.contains(i)) c++;
  }
  return c + s.length;
}

final keys = List<String>.generate(20000, (i) => 'name_$i');

int mapStr(int rounds) {
  var t = 0;
  for (var r = 0; r < rounds; r++) {
    final m = <String, int>{};
    for (var i = 0; i < keys.length; i++) {
      final k = keys[(i * 7) % keys.length];
      m[k] = (m[k] ?? 0) + i;
    }
    t = (t + m.length + (m['name_7'] ?? 0)) & 0x3FFFFFFF;
  }
  return t;
}

int mapIterate(Map<String, int> m) {
  var s = 0;
  for (var r = 0; r < 20; r++) {
    for (final e in m.entries) {
      s = (s + e.key.length + e.value) & 0x3FFFFFFF;
    }
    m.forEach((k, v) => s = (s + v) & 0x3FFFFFFF);
    for (final v in m.values) {
      s = (s ^ v) & 0x3FFFFFFF;
    }
  }
  return s;
}

int listMethods(List<int> l) {
  var s = 0;
  for (var r = 0; r < 10; r++) {
    final evens = l.where((x) => x.isEven).map((x) => x * 3).toList();
    s += evens.length + evens.last;
    s += l.contains(-1) ? 1 : 0;
    s += l.indexOf(l.length - 1);
    s += l.sublist(10, 1000).fold(0, (a, b) => a + b);
    final copy = <int>[]..addAll(l);
    while (copy.length > l.length - 1000) {
      s += copy.removeLast();
    }
    s += copy.join(',').length;
  }
  return s & 0x3FFFFFFF;
}

int mapRemove(int n) {
  final m = <int, int>{};
  for (var i = 0; i < n; i++) {
    m[i] = i;
  }
  for (var i = 0; i < n; i += 2) {
    m.remove(i);
  }
  var s = 0;
  m.forEach((k, v) => s += v);
  return s + m.length;
}

class Key {
  final int a, b;
  const Key(this.a, this.b);
  @override
  bool operator ==(Object o) => o is Key && o.a == a && o.b == b;
  @override
  int get hashCode => a * 31 + b;
}

int mapObjectKey(int n) {
  final m = <Key, int>{};
  for (var i = 0; i < n; i++) {
    m[Key(i % 1000, i ~/ 1000)] = i;
  }
  var s = 0;
  for (var i = 0; i < n; i++) {
    s = (s + (m[Key(i % 1000, i ~/ 1000)] ?? 0)) & 0x3FFFFFFF;
  }
  return s;
}

void main() {
  final m = <String, int>{for (var i = 0; i < 20000; i++) keys[i]: i};
  final l = List<int>.generate(100000, (i) => i);
  measure('set_int', () => setInt(400000));
  measure('map_str_int', () => mapStr(10));
  measure('map_iterate', () => mapIterate(m));
  measure('list_methods', () => listMethods(l));
  measure('map_remove', () => mapRemove(300000));
  measure('map_object_key', () => mapObjectKey(200000));
}
