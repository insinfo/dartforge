// Caminho rápido do `_Map`/`_Set` padrão para chaves `int` e `String`
// (`crates/runtime/src/nativos_hash.rs`): a mesma tabela do SDK, então a
// ordem de inserção, as remoções, `int` diante de `double` (`1 == 1.0`),
// `_Mint`, strings de um e de dois bytes e chaves de outros tipos na mesma
// tabela têm de dar o que a VM dá.
class P {
  final int x;
  P(this.x);
  bool operator ==(Object o) => o is P && o.x == x;
  int get hashCode => x % 7;
  String toString() => 'P$x';
}

void main() {
  final m = <Object?, Object?>{};
  m[1] = 'um';
  m[1.0] = 'um double';
  m[2.0] = 'dois double';
  m[2] = 'dois int';
  m['a'] = 1;
  m['a' + 'b'] = 2;
  m['ab'] = 3;
  m[null] = 'nulo';
  m[P(3)] = 'p3';
  m[P(10)] = 'p10';
  m[P(3)] = 'p3b';
  m[1 << 62] = 'grande';
  m[(1 << 62) + 1] = 'grande1';
  m[-(1 << 62) - 5] = 'negativo';
  m[9223372036854775807] = 'max';
  m['ção'] = 'dois bytes';
  m[String.fromCharCodes([0x100, 0x41])] = 'Ā';
  m[true] = 'verdade';
  m[1.5] = 'um e meio';
  print(m);
  print(m.length);
  print([m[1], m[1.0], m[2], m['ab'], m['a'], m[null], m[P(3)], m[P(17)], m[1 << 62], m[9223372036854775807], m['ção'], m['Ā'.substring(0, 1) + 'A'], m[true], m[1.5], m[3]]);
  print([m.containsKey(2), m.containsKey(2.0), m.containsKey('x'), m.containsKey(-(1 << 62) - 5)]);
  m.remove('a');
  m.remove(2);
  m['a'] = 'de novo';
  m[2] = 'dois de novo';
  print(m.keys.toList());
  print(m.values.toList());
  for (var i = 0; i < 100; i++) {
    m[i] = i * i;
    m['k$i'] = i;
  }
  for (var i = 0; i < 100; i += 3) {
    m.remove(i);
    m.remove('k$i');
  }
  var s = 0;
  m.forEach((k, v) => s += (v is int ? v : 0));
  print(s);
  print(m.length);
  print(m.keys.take(30).toList());
  final mi = <int, int>{};
  for (var i = 0; i < 5000; i++) {
    mi[i * 7919 % 5000] = i;
  }
  var t = 0;
  for (var i = 0; i < 5000; i++) {
    t += mi[i]!;
  }
  print(t);
  print(mi.keys.take(10).toList());
  print(mi.putIfAbsent(4999, () => -1));
  print(mi.update(3, (v) => v + 1));

  final c = <Object?>{};
  print([c.add(1), c.add(1.0), c.add('x'), c.add('x' * 1), c.add(null), c.add(P(1)), c.add(P(8)), c.add(2.0), c.add(2)]);
  print(c);
  print([c.contains(1), c.contains(1.0), c.contains(2), c.contains('x'), c.contains(P(8)), c.lookup(2), c.lookup(1.0), c.lookup('x')]);
  final cs = <String>{};
  for (var i = 0; i < 3000; i++) {
    cs.add('s${i % 1000}');
  }
  cs.remove('s5');
  cs.add('s5');
  print(cs.length);
  print(cs.take(8).toList());
  print(cs.last);
  print(cs.contains('s999'));
  print(cs.lookup('s10'));
  final ci = <int>{for (var i = 0; i < 40; i++) i % 13};
  print(ci);
  print('abc'.hashCode);
  print('ção'.hashCode);
  print(m.hashCode == m.hashCode);
  final im = Map<Object?, int>.identity();
  im['a'] = 1;
  im['a' + ''] = 2;
  print(im.length);
  final lm = <num, String>{};
  lm[3.0] = 'double';
  lm[3] = 'int';
  print(lm);
  final lset = <num>{3.0};
  print(lset.add(3));
  print(lset);
  try {
    final Map<Object, int> mm = <String, int>{};
    mm[1] = 2;
  } catch (e) {
    print('erro de tipo: ${e.runtimeType}');
  }
}
