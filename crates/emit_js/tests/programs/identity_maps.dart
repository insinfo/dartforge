// The maps with primitive keys (`IdentityMap` in the JavaScript runtime):
// insertion order, the three kinds of keys (strings, small integers, the
// rest), `-0.0`, `NaN`, `null` and `bool` keys, removal, `clear`,
// `putIfAbsent`, `addAll`, the iterables of keys and values and the
// concurrent modification checks. Every line prints the same on the VM.
import 'dart:collection';
import 'dart:convert';

class Key {
  final int id;
  Key(this.id);
  @override
  bool operator ==(Object other) => other is Key && other.id == id;
  @override
  int get hashCode => id;
  @override
  String toString() => 'Key($id)';
}

String error(void Function() f) {
  try {
    f();
    return 'no error';
  } on ConcurrentModificationError {
    return 'ConcurrentModificationError';
  } catch (e) {
    return e.runtimeType.toString();
  }
}

void order() {
  final m = <int, String>{};
  for (final k in [5, 1000000, 3, 0, 1 << 30, 7, 2147483647, 1]) {
    m[k] = 'v$k';
  }
  m[3] = 'again';
  print(m);
  m.remove(5);
  m[5] = 'back';
  print(m.keys.toList());
  print(m.values.toList());
  print('${m.length} ${m.isEmpty} ${m.isNotEmpty} ${m[3]} ${m[4]} ${m[1 << 30]}');
  final neg = <int, int>{-1: 1, -1000000: 2, 9007199254740991: 3, 0: 4};
  neg[-1] = 10;
  print('$neg ${neg[-1]} ${neg[9007199254740991]} ${neg.containsKey(-2)}');
  neg.remove(-1000000);
  print('$neg ${neg.length}');
}

void strings() {
  final m = <String, int>{};
  for (final k in ['b', '__proto__', 'constructor', 'toString', '0', '', 'a', 'hasOwnProperty', '10']) {
    m[k] = k.length;
  }
  m['b'] = 99;
  print(m);
  print('${m['__proto__']} ${m['constructor']} ${m['valueOf']} ${m['0']} ${m['']}');
  print('${m.containsKey('toString')} ${m.containsKey('toLocaleString')} ${m.containsValue(99)} ${m.containsValue(100)}');
  m.remove('__proto__');
  m.remove('missing');
  print('${m.keys.join(',')} ${m.length}');
  m.clear();
  m.clear();
  print('$m ${m.length} ${m['b']}');
  m['x'] = 1;
  print(m);
}

void mixed() {
  final m = <Object?, String>{};
  final key = Key(1);
  m[1] = 'int';
  m['1'] = 'string';
  m[null] = 'null';
  m[true] = 'true';
  m[false] = 'false';
  m[-5] = 'negative';
  m[key] = 'key';
  m[2.5] = 'double';
  print(m);
  print('${m[1]} ${m['1']} ${m[null]} ${m[true]} ${m[false]} ${m[-5]} ${m[key]} ${m[Key(1)]} ${m[2.5]}');
  m.remove(null);
  m.remove(true);
  m.remove('1');
  print(m);
  print(m.keys.map((k) => k.runtimeType == Key ? 'Key' : '$k').join(' '));
}

void doubles() {
  final z = <double, String>{};
  z[0.0] = 'a';
  z[-0.0] = 'b';
  print('${z.length} ${z[0.0]} ${z[-0.0]} ${z.keys.first.isNegative}');
  final n = <num, String>{};
  n[-0.0] = 'a';
  n[0] = 'b';
  print('${n.length} ${n[0]} ${n[-0.0]} ${n.keys.first.isNegative}');
  final nan = <double, int>{};
  nan[double.nan] = 1;
  nan[double.nan] = 2;
  nan[1.5] = 3;
  print('${nan.length} ${nan[double.nan]} ${nan.containsKey(double.nan)} ${nan.remove(double.nan)} ${nan.length}');
  print(nan.values.toList());
  nan.forEach((k, v) => print('${k.isNaN} $v'));
  nan.clear();
  print('${nan.length} ${nan.values.toList()}');
  final one = <num, String>{1: 'a'};
  one[1.0] = 'b';
  print('${one.length} ${one[1]} ${one.keys.first is int}');
}

void nullable() {
  final m = <int?, int?>{null: null, 1: null};
  print('${m.length} ${m[null]} ${m.containsKey(null)} ${m.containsKey(2)} ${m.containsValue(null)}');
  m[null] = 3;
  print(m);
}

void iteration() {
  final m = <int, int>{1: 1, 2: 2, 3: 3};
  print(error(() {
    for (final k in m.keys) {
      m[k] = k * 10;
    }
  }));
  print(m);
  print(error(() {
    for (final k in m.keys) {
      if (k == 2) m[4] = 4;
    }
  }));
  print(error(() {
    for (final v in m.values) {
      if (v == 10) m.remove(3);
    }
  }));
  print(error(() {
    m.forEach((k, v) {
      if (k == 1) m[9] = 9;
    });
  }));
  print(error(() {
    m.forEach((k, v) {
      m[k] = v + 1;
    });
  }));
  print(m);
  print(error(() {
    for (final e in m.entries) {
      if (e.key == 1) m.clear();
    }
  }));
  print('$m ${m.length}');
  final i = <String, int>{'a': 1}.keys.iterator;
  print('${i.moveNext()} ${i.current} ${i.moveNext()} ${i.moveNext()}');
  final j = <String, int>{'a': 1};
  final it = j.values.iterator;
  print(it.moveNext());
  j['b'] = 2;
  print(error(() => it.moveNext()));
  final done = <int, int>{1: 1};
  final end = done.keys.iterator;
  while (end.moveNext()) {}
  done.remove(1);
  print(error(() => end.moveNext()));
  final keys = <int, String>{3: 'c', 1: 'a'}.keys;
  print('${keys.length} ${keys.contains(1)} ${keys.contains(2)} ${keys.first} ${keys.last} ${keys.isEmpty}');
  final values = <int, String>{3: 'c', 1: 'a'}.values;
  print('${values.contains('a')} ${values.contains('b')} ${values.join('+')}');
}

void operations() {
  final m = <String, int>{'a': 1};
  print(m.putIfAbsent('a', () => 5));
  print(m.putIfAbsent('b', () => 2));
  print(m.putIfAbsent('c', () {
    m['c'] = 30;
    m['d'] = 4;
    return 3;
  }));
  print(m);
  m.addAll({'a': 10, 'e': 5});
  print(m);
  print(error(() {
    for (final k in m.keys) {
      if (k == 'a') m.addAll({'a': 11});
    }
  }));
  print(m);
  m.update('b', (v) => v + 100);
  m.update('z', (v) => v, ifAbsent: () => 26);
  m.updateAll((k, v) => v * 2);
  print(m);
  m.removeWhere((k, v) => v > 50);
  print('$m ${Map.of(m)} ${Map<String, int>.from(m)}');
  print(m.map((k, v) => MapEntry(v, k)));
  print(jsonEncode(m));
  final decoded = jsonDecode('{"x": 1, "y": {"z": [1, 2]}, "__proto__": 3}') as Map<String, dynamic>;
  decoded['w'] = 0;
  print('$decoded ${decoded['__proto__']}');
  final literal = <String, int>{'k': 1, 'j': 2, 'k2': 3};
  print(literal.entries.map((e) => '${e.key}=${e.value}').join('&'));
}

void identityMaps() {
  final a = Key(1), b = Key(1);
  final m = Map<Key, int>.identity();
  m[a] = 1;
  m[b] = 2;
  print('${m.length} ${m[a]} ${m[b]} ${m[Key(1)]}');
  final h = HashMap<int, String>();
  h[2] = 'b';
  h[1] = 'a';
  print('${h.length} ${h[1]} ${h[2]}');
  final l = LinkedHashMap<String, int>.identity();
  l['x'] = 1;
  l['x'] = 2;
  print(l);
  final custom = LinkedHashMap<int, int>(equals: (x, y) => x % 10 == y % 10, hashCode: (x) => x % 10);
  custom[1] = 1;
  custom[11] = 2;
  print(custom);
}

void many() {
  final m = <int, int>{};
  for (var i = 0; i < 5000; i++) {
    m[i * 7 % 5000] = i;
  }
  for (var i = 0; i < 5000; i += 3) {
    m.remove(i);
  }
  var s = 0;
  for (final e in m.entries) {
    s = (s * 31 + e.key * 7 + e.value) % 1000000007;
  }
  print('${m.length} $s ${m.keys.take(5).toList()}');
  final w = <String, int>{};
  for (var i = 0; i < 3000; i++) {
    w['w${i % 700}'] = (w['w${i % 700}'] ?? 0) + i;
  }
  print('${w.length} ${w['w0']} ${w['w699']} ${w.keys.skip(695).toList()}');
}

void main() {
  order();
  strings();
  mixed();
  doubles();
  nullable();
  iteration();
  operations();
  identityMaps();
  many();
}
