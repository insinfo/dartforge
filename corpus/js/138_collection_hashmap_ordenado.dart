// HashMap/HashSet impressos sempre ordenados: containsKey/Value, remove, fromIterable/fromEntries, entries, union/intersection/difference.
import 'dart:collection';

List<T> ord<T extends Comparable>(Iterable<T> xs) => xs.toList()..sort();
String mapaOrd<K extends Comparable, V>(Map<K, V> m) =>
    (m.keys.toList()..sort()).map((k) => '$k:${m[k]}').join(', ');

void main() {
  var hm = HashMap<String, int>();
  hm['banana'] = 2;
  hm['abacaxi'] = 1;
  hm['cereja'] = 3;
  print(ord(hm.keys));
  print(ord(hm.values));
  print(mapaOrd(hm));
  print(hm.length);
  print(hm.containsKey('banana'));
  print(hm.containsKey('uva'));
  print(hm.containsValue(3));
  print(hm.containsValue(9));
  print(hm['abacaxi']);
  print(hm['uva']);
  print(hm.remove('banana'));
  print(hm.remove('banana'));
  print(mapaOrd(hm));
  hm['banana'] = 20;
  print(mapaOrd(hm));
  print(hm.putIfAbsent('uva', () => 4));
  print(hm.putIfAbsent('uva', () => 40));
  print(hm.update('uva', (v) => v * 10));
  print(hm.update('kiwi', (v) => v, ifAbsent: () => 5));
  hm.updateAll((k, v) => v + 1);
  print(mapaOrd(hm));
  hm.removeWhere((k, v) => v > 10);
  print(mapaOrd(hm));
  hm.addAll({'x': 1, 'y': 2});
  hm.addEntries([MapEntry('z', 3)]);
  print(mapaOrd(hm));
  print(ord(hm.entries.map((e) => '${e.key}=${e.value}')));
  print(hm.isEmpty);
  print(hm.isNotEmpty);
  var soma = 0;
  hm.forEach((k, v) => soma += v);
  print(soma);
  print(hm.values.fold<int>(0, (a, b) => a + b));
  print(ord(hm.keys.where((k) => k.length == 1)));
  var copia = HashMap.of(hm);
  copia['novo'] = 0;
  print(hm.containsKey('novo'));
  print(copia.length - hm.length);
  hm.clear();
  print(hm.isEmpty);
  print(hm.length);

  var fi = HashMap<String, int>.fromIterable([1, 2, 3], key: (e) => 'k$e', value: (e) => (e as int) * e);
  print(mapaOrd(fi));
  var fi2 = HashMap<String, int>.fromIterable(['a', 'bb', 'ccc'], value: (e) => (e as String).length);
  print(mapaOrd(fi2));
  var fi3 = HashMap<int, int>.fromIterable([1, 2]);
  print(mapaOrd(fi3));
  var fe = HashMap.fromEntries([MapEntry('um', 1), MapEntry('dois', 2), MapEntry('um', 11)]);
  print(mapaOrd(fe));
  print(fe.length);
  var fis = HashMap.fromIterables(['a', 'b', 'c'], [1, 2, 3]);
  print(mapaOrd(fis));
  var mfi = Map<int, int>.fromIterable([3, 1, 2], key: (e) => e, value: (e) => (e as int) * 10);
  print(mapaOrd(mfi));
  var mfe = Map.fromEntries({'z': 1, 'a': 2}.entries);
  print(mapaOrd(mfe));
  print(mapaOrd(HashMap<int, String>.from({1: 'a', 2: 'b'})));
  var hmInt = HashMap<int, int>();
  for (var i = 10; i >= 0; i--) {
    hmInt[i] = i * i;
  }
  print(ord(hmInt.keys));
  print(ord(hmInt.values));
  print(ord(hmInt.entries.where((e) => e.value.isEven).map((e) => e.key)));
  var hmObj = HashMap<Object?, int>();
  hmObj[1] = 1;
  hmObj['1'] = 2;
  hmObj[true] = 3;
  hmObj[null] = 4;
  print(hmObj.length);
  print(hmObj[1]);
  print(hmObj['1']);
  print(hmObj[true]);
  print(hmObj[null]);
  print(hmObj.containsKey(null));
  var hmCI = HashMap<String, int>(
      equals: (a, b) => a.toLowerCase() == b.toLowerCase(),
      hashCode: (s) => s.toLowerCase().hashCode);
  hmCI['Abc'] = 1;
  hmCI['ABC'] = 2;
  hmCI['def'] = 3;
  print(hmCI.length);
  print(hmCI['abc']);
  print(ord(hmCI.values));
  var hmId = HashMap<String, int>.identity();
  hmId['a'] = 1;
  print(hmId.length);

  var hs = HashSet<int>();
  hs.addAll([5, 3, 9, 3, 5, 1]);
  print(ord(hs));
  print(hs.length);
  print(hs.contains(9));
  print(hs.contains(4));
  print(hs.add(4));
  print(hs.add(4));
  print(hs.remove(9));
  print(hs.remove(9));
  print(ord(hs));
  print(hs.lookup(3));
  print(hs.lookup(99));
  hs.removeAll([1, 3]);
  print(ord(hs));
  hs.retainAll([4, 5, 100]);
  print(ord(hs));
  hs.removeWhere((e) => e == 4);
  print(ord(hs));
  hs.retainWhere((e) => e > 100);
  print(hs.isEmpty);
  var a = HashSet<int>.from([1, 2, 3, 4]);
  var b = HashSet<int>.of([3, 4, 5, 6]);
  print(ord(a.union(b)));
  print(ord(a.intersection(b)));
  print(ord(a.difference(b)));
  print(ord(b.difference(a)));
  print(ord(a.union(b).difference(a.intersection(b))));
  print(a.containsAll([1, 2]));
  print(a.containsAll([1, 9]));
  print(a.containsAll(<int>[]));
  print(ord(a.where((e) => e.isEven)));
  print(ord(a.map((e) => e * 3)));
  print(ord(a.toList()));
  print(ord(a.toSet()));
  print(ord(HashSet<String>.from('banana'.split(''))));
  print(HashSet<String>.from('banana'.split('')).length);
  print(ord(Set<int>.from([3, 1, 2])));
  print(ord(Set<String>.of(['b', 'a'])));
  print(ord({1, 2}.union({2, 3})));
  print(ord({1, 2, 3}.intersection({2, 3, 4})));
  print(ord({1, 2, 3}.difference({2})));
  print(ord(HashSet<int>.identity()..add(1)..add(1)));
  var hsCI = HashSet<String>(
      equals: (x, y) => x.toLowerCase() == y.toLowerCase(),
      hashCode: (s) => s.toLowerCase().hashCode);
  hsCI.addAll(['A', 'a', 'B']);
  print(hsCI.length);
  print(hsCI.contains('b'));
  var hsObj = HashSet<Object?>.from([1, '1', null, 1.5, true]);
  print(hsObj.length);
  print(hsObj.contains(null));
  print(hsObj.contains('1'));
  print(hsObj.contains(2));
  var contagem = HashMap<String, int>();
  for (var p in 'o rato roeu a roupa do rei de roma'.split(' ')) {
    contagem[p] = (contagem[p] ?? 0) + 1;
  }
  print(mapaOrd(contagem));
  var repetidas = contagem.entries.where((e) => e.value > 1).map((e) => e.key);
  print(ord(repetidas));
  var porTamanho = HashMap<int, HashSet<String>>();
  for (var p in contagem.keys) {
    porTamanho.putIfAbsent(p.length, () => HashSet()).add(p);
  }
  for (var k in ord(porTamanho.keys)) {
    print('$k: ${ord(porTamanho[k]!)}');
  }
  print(HashMap<String, int>().isEmpty);
  print(HashSet<int>().isEmpty);
  print(ord(HashSet<int>()).isEmpty);
  print(hm is Map<String, int>);
  print(hs is Set<int>);
  print(a.toList().length == a.length);
  print(a.length == a.toSet().length);
  print(a.first == a.first);
  print(a.reduce((x, y) => x + y));
  print(a.fold<int>(0, (x, y) => x + y));
  print(a.any((e) => e > 3));
  print(a.every((e) => e > 0));
  print(HashSet<int>.from([7]).single);
}
