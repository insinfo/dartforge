// dart:collection: LinkedHashMap com ordem de inserção, SplayTreeMap/SplayTreeSet ordenados, UnmodifiableListView e MapView.
import 'dart:collection';

void main() {
  var lhm = LinkedHashMap<String, int>();
  lhm['c'] = 3;
  lhm['a'] = 1;
  lhm['b'] = 2;
  print(lhm);
  print(lhm.keys.toList());
  lhm.remove('a');
  lhm['a'] = 10;
  print(lhm);
  lhm['c'] = 30;
  print(lhm.keys.toList());
  print(lhm.putIfAbsent('d', () => 4));
  print(lhm.putIfAbsent('d', () => 400));
  print(lhm);
  print(lhm.update('d', (v) => v + 1));
  print(lhm.update('z', (v) => v + 1, ifAbsent: () => 0));
  lhm.updateAll((k, v) => v * 2);
  print(lhm);
  print(lhm.containsKey('z'));
  print(lhm.containsValue(20));
  print(lhm.length);
  print(lhm.entries.map((e) => '${e.key}=${e.value}').join(';'));
  lhm.removeWhere((k, v) => v > 10);
  print(lhm);
  lhm.addAll({'x': 1, 'y': 2});
  lhm.addEntries([MapEntry('w', 0)]);
  print(lhm);
  print(lhm.values.toList());
  var lhm2 = LinkedHashMap.from({'k': 'v'});
  print(lhm2);
  var lhm3 = LinkedHashMap<String, int>(
      equals: (a, b) => a.toLowerCase() == b.toLowerCase(),
      hashCode: (s) => s.toLowerCase().hashCode);
  lhm3['A'] = 1;
  lhm3['a'] = 2;
  print(lhm3);
  print(lhm3.length);
  print(lhm3['A']);

  var stm = SplayTreeMap<String, int>();
  stm['banana'] = 2;
  stm['abacaxi'] = 1;
  stm['cereja'] = 3;
  print(stm);
  print(stm.firstKey());
  print(stm.lastKey());
  print(stm.keys.toList());
  print(stm.firstKeyAfter('abacaxi'));
  print(stm.lastKeyBefore('cereja'));
  print(stm.firstKeyAfter('cereja'));
  print(stm.lastKeyBefore('abacaxi'));
  stm['aaa'] = 0;
  print(stm.keys.toList());
  stm.remove('banana');
  print(stm);
  var inv = SplayTreeMap<int, String>((a, b) => b.compareTo(a));
  inv[1] = 'um';
  inv[3] = 'tres';
  inv[2] = 'dois';
  print(inv);
  print(inv.firstKey());
  print(inv.lastKey());
  var porLen = SplayTreeMap<String, int>(
      (a, b) => a.length != b.length ? a.length.compareTo(b.length) : a.compareTo(b));
  porLen['ccc'] = 3;
  porLen['a'] = 1;
  porLen['bb'] = 2;
  porLen['aa'] = 22;
  print(porLen);
  print(porLen.keys.toList());
  var stm2 = SplayTreeMap<int, int>.from({5: 50, 1: 10, 3: 30});
  print(stm2);
  print(stm2.entries.first.key);
  print(stm2.entries.last.value);
  print(SplayTreeMap<String, int>().isEmpty);
  print(SplayTreeMap<String, int>().firstKey());

  var sts = SplayTreeSet<int>();
  sts.addAll([5, 3, 9, 1, 3, 5]);
  print(sts);
  print(sts.length);
  print(sts.first);
  print(sts.last);
  print(sts.toList());
  print(sts.contains(9));
  sts.remove(9);
  print(sts);
  print(sts.lookup(3));
  print(sts.lookup(4));
  var stsInv = SplayTreeSet<String>((a, b) => b.compareTo(a));
  stsInv.addAll(['b', 'a', 'c']);
  print(stsInv);
  var stsCI = SplayTreeSet<String>(
      (a, b) => a.toLowerCase().compareTo(b.toLowerCase()));
  stsCI.addAll(['B', 'a', 'A', 'b']);
  print(stsCI);
  print(stsCI.length);
  print(SplayTreeSet.from([3, 1, 2]).join('-'));
  print(sts.union(SplayTreeSet.from([100, 0])));
  print(sts.intersection({1, 5, 77}));
  print(sts.difference({1}));
  print(sts.where((e) => e > 1).toList());
  print(sts.map((e) => e * 2).toList());

  var lista = [1, 2, 3];
  var view = UnmodifiableListView(lista);
  print(view);
  print(view.length);
  print(view[0]);
  print(view.first);
  print(view.reversed.toList());
  try {
    view.add(4);
  } catch (e) {
    print('lançou ${e is UnsupportedError}');
  }
  try {
    view[0] = 9;
  } catch (e) {
    print('lançou ${e is UnsupportedError}');
  }
  lista.add(4);
  print(view);
  print(view.length);
  var mv = MapView({'a': 1});
  print(mv);
  print(mv['a']);
  print(mv.keys.toList());
  print(mv.length);
  var umv = UnmodifiableMapView({'k': 1});
  print(umv);
  try {
    umv['z'] = 2;
  } catch (e) {
    print('lançou ${e is UnsupportedError}');
  }
  var usv = UnmodifiableSetView({1, 2});
  print(usv);
  try {
    usv.add(3);
  } catch (e) {
    print('lançou ${e is UnsupportedError}');
  }
  print(Map.unmodifiable({'a': 1}));
  print(List.unmodifiable([1, 2]));
  try {
    List.unmodifiable([1]).clear();
  } catch (e) {
    print('lançou ${e is UnsupportedError}');
  }
}
