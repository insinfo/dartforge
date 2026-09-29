// N14: a `List<int>`, `List<double>` e `List<bool>` do runtime guardam os
// elementos compactos (8 bytes, sem tag nem referência). A forma é
// invisível: tipos reificados, covariância, TypeError na inserção, null só
// em `List<int?>`, ordenação, fatias, cópias, iteradores, igualdade, o
// coletor com listas grandes e a cópia entre isolados têm de dar o mesmo que
// a VM.
import 'dart:isolate';

void tentar(String nome, void Function() f) {
  try {
    f();
    print('$nome: ok');
  } on TypeError catch (e) {
    print('$nome: TypeError ${e.runtimeType}');
  } catch (e) {
    print('$nome: ${e.runtimeType} $e');
  }
}

int soma(List<int> v) {
  var s = 0;
  for (var i = 0; i < v.length; i++) {
    s += v[i];
  }
  return s;
}

void incrementar(List<int> v) {
  for (var i = 0; i < v.length; i++) {
    v[i] = v[i] + 1;
  }
}

double somaD(List<double> v) {
  var s = 0.0;
  for (var i = 0; i < v.length; i++) {
    s += v[i];
  }
  return s;
}

int contar(List<bool> v) {
  var c = 0;
  for (var i = 0; i < v.length; i++) {
    if (v[i]) c++;
  }
  return c;
}

/// A mesma lista vista como `List<num>`: as voltas de N13 da forma geral
/// não servem para a compacta; tem de dar o mesmo.
num somaNum(List<num> v) {
  num s = 0;
  for (var i = 0; i < v.length; i++) {
    s += v[i];
  }
  return s;
}

Object? primeiro(List<Object?> v) => v.isEmpty ? null : v[0];

void tipos() {
  final a = <int>[1, 2, 3];
  final b = List<double>.filled(2, 0.5);
  final c = List<bool>.generate(3, (i) => i.isEven);
  print('${a.runtimeType} ${b.runtimeType} ${c.runtimeType}');
  print('${a is List<int>} ${a is List<num>} ${a is List<int?>} ${a is List<double>}');
  print('${b is List<double>} ${b is List<num>} ${c is List<bool>} ${c is List<Object>}');
  final Object o = a;
  print('${(o as List<num>).length} ${o is List<String>}');
  tentar('as List<String>', () => o as List<String>);
  print(<int>[].runtimeType);
  print(List<int>.empty(growable: true).runtimeType);
  print(List<int>.filled(0, 0, growable: true).runtimeType);
  print(List<int>.unmodifiable([1]).runtimeType);
  print(const <int>[1, 2].runtimeType);
}

void covariancia() {
  final List<Object?> o = <int>[1, 2];
  tentar('add String', () => o.add('x'));
  tentar('add null', () => o.add(null));
  tentar('[]= double', () => o[0] = 1.5);
  tentar('[]= null', () => o[1] = null);
  tentar('insert bool', () => o.insert(0, true));
  tentar('addAll', () => o.addAll(<Object>['a']));
  tentar('add int', () => o.add(3));
  print(o);
  final List<num> n = <double>[1.0];
  tentar('double recebe int', () => n.add(2));
  tentar('double recebe double', () => n.add(2.5));
  print(n);
  final List<Object> bs = <bool>[true];
  tentar('bool recebe int', () => bs[0] = 0);
  print(bs);
  final dynamic d = <int>[4, 5];
  tentar('dynamic []= String', () => d[0] = 'a');
  tentar('dynamic add double', () => d.add(1.0));
  tentar('dynamic add int', () => d.add(6));
  print(d);
  final l = <int>[1];
  tentar('length cresce', () => l.length = 3);
  print(l);
  final anul = <int?>[1, null];
  anul.length = 4;
  anul[3] = 9;
  print(anul);
  print(primeiro(<int>[7]));
  print(primeiro(<double>[7]));
  print(primeiro(<bool>[false]));
}

void estrutura() {
  final l = <int>[];
  for (var i = 0; i < 20; i++) {
    l.add(i * i - 50);
  }
  print(l.removeLast());
  print(l.removeAt(3));
  l.remove(-49);
  l.insert(2, 1000);
  l.insertAll(1, [7, 8, 9]);
  l.removeRange(0, 2);
  l.removeWhere((x) => x.isOdd);
  l.retainWhere((x) => x != 14);
  print(l);
  l.length = 3;
  print(l);
  l.clear();
  l.add(5);
  print('$l ${l.length}');
  final f = List<int>.filled(6, 3);
  f.setRange(1, 4, [9, 8, 7]);
  f.fillRange(4, 6, -1);
  print(f);
  tentar('fixa add', () => f.add(1));
  final u = List<int>.unmodifiable(f);
  tentar('imutavel []=', () => u[0] = 1);
  print(soma(u));
  const k = <int>[5, 6, 7];
  print(soma(k));
  tentar('const []=', () => (k as List<int>)[0] = 1);
  final g = List<double>.generate(4, (i) => i / 2, growable: false);
  print(g);
}

void ordenar() {
  const mn = -9223372036854775808, mx = 9223372036854775807;
  final l = <int>[5, mx, -3, 1 << 62, 0, mn, (1 << 62) - 1, -(1 << 62) - 1, 5];
  l.sort();
  print(l);
  l.sort((a, b) => b.compareTo(a));
  print(l);
  final d = <double>[2.5, -0.0, 0.0, double.infinity, -1e300, double.negativeInfinity, 1e-300];
  d.sort();
  print(d);
  final bs = <bool>[true, false, true];
  bs.sort((a, b) => a == b ? 0 : (a ? 1 : -1));
  print(bs);
  final r = List<int>.generate(1000, (i) => (i * 7919) % 1009 - 500)..sort();
  var ok = true;
  for (var i = 1; i < r.length; i++) {
    if (r[i - 1] > r[i]) ok = false;
  }
  print('ordenada $ok ${r.first} ${r.last}');
  final s = <int>[3, 1, 2]..shuffle();
  s.sort();
  print(s);
}

void copias() {
  final a = <int>[1, 2, 3, 4, 5];
  final s = a.sublist(1, 4);
  s[0] = 20;
  print('$a $s ${s.runtimeType}');
  print(a.getRange(1, 3).toList());
  print(a.reversed.toList());
  final b = <int>[...a, 6];
  b.addAll(a);
  b.addAll([7, 8]);
  b.addAll(Iterable<int>.generate(2, (i) => 100 + i));
  print(b);
  print(a + [9]);
  final List<num> misto = <num>[1, 2.5];
  misto.addAll(a);
  print(misto);
  final c = List<int>.of(a);
  c[0] = -1;
  final t = a.toList(growable: false);
  tentar('toList fixa add', () => t.add(1));
  print('$a $c $t');
  final dyn = <dynamic>[1, 2, 3];
  final dl = List<int>.from(dyn);
  dl.add(4);
  print('$dl ${dl.runtimeType}');
  tentar('from com String', () => List<int>.from(<dynamic>[1, 'a']));
  final d = <double>[0.5, 1.5];
  final e = [...d, ...d.map((x) => x * 2)];
  print('$e ${e.runtimeType}');
  print(a.asMap());
  print(a.expand((x) => [x, -x]).take(4).toList());
}

void iteracao() {
  final a = <int>[10, 20, 30];
  var t = 0;
  for (final x in a) {
    t += x;
  }
  final it = a.iterator;
  while (it.moveNext()) {
    t += it.current;
  }
  print(t);
  print(a.map((x) => x + 1).where((x) => x > 11).fold<int>(0, (p, x) => p + x));
  print('${a.join('-')} ${a.contains(20)} ${a.indexOf(30)} ${a.lastIndexOf(5)}');
  try {
    for (final x in a) {
      if (x == 10) a.add(40);
    }
  } on ConcurrentModificationError {
    print('modificar no for-in: ConcurrentModificationError $a');
  }
  final d = <double>[double.nan, -0.0, 0.0];
  print('${d.contains(double.nan)} ${d.indexOf(0.0)} ${d.indexOf(-0.0)} $d');
  final bs = <bool>[true, false, true];
  print('${bs.every((b) => b)} ${bs.any((b) => !b)} ${bs.where((b) => b).length}');
  print('${contar(bs)} ${somaD(d.sublist(1))}');
}

void igualdade() {
  final a = <int>[1, 2];
  final b = <int>[1, 2];
  print('${a == b} ${a == a} ${identical(a, b)}');
  print(a.hashCode == a.hashCode);
  final m = <List<int>, String>{a: 'a'};
  print('${m[a]} ${m[b]}');
  final conj = {<int>[1]};
  print(conj.contains(<int>[1]));
}

void lacos() {
  final v = List<int>.generate(10, (i) => i);
  incrementar(v);
  print(soma(v));
  print(somaNum(v));
  print(somaNum(<double>[0.5, 0.25]));
  print(somaNum(<num>[1, 0.5]));
  final List<int> geral = <int>[1, 2, 3].cast<int>();
  print(soma(geral));
  final grande = <int>[1 << 62, -(1 << 62) - 1, 9007199254740993];
  incrementar(grande);
  print(grande);
  final vazia = <int>[];
  print(soma(vazia));
  final dd = List<double>.filled(3, 1.0);
  for (var i = 0; i < dd.length; i++) {
    dd[i] = dd[i] * i + 0.5;
  }
  print(dd);
  final bb = List<bool>.filled(4, false);
  for (var i = 0; i < bb.length; i++) {
    bb[i] = i.isOdd;
  }
  print(bb);
}

/// Listas grandes com o coletor: nada nelas é referência, e as listas de
/// listas continuam vivas enquanto alcançáveis.
void coletor() {
  final vivas = <List<int>>[];
  var total = 0;
  for (var r = 0; r < 6; r++) {
    final l = List<int>.generate(50000, (i) => i ^ r);
    final d = List<double>.filled(20000, r + 0.5);
    final lixo = List<Object>.generate(200, (i) => 'x$i');
    total += soma(l) + d.length + lixo.length;
    if (r.isEven) vivas.add(l);
  }
  final cresce = <int>[];
  for (var i = 0; i < 100000; i++) {
    cresce.add(i);
  }
  final textos = <String>[for (var i = 0; i < 100; i++) 't$i'];
  print('$total ${vivas.length} ${soma(vivas[2])} ${soma(cresce)} ${textos[99]}');
  final obj = <Object>[vivas[0], <double>[1.5], <bool>[true]];
  print('${(obj[0] as List<int>)[49999]} ${obj[1]} ${obj[2]}');
}

Future<void> main() async {
  tipos();
  covariancia();
  estrutura();
  ordenar();
  copias();
  iteracao();
  igualdade();
  lacos();
  coletor();
  final l = await Isolate.run(() => List<int>.generate(5, (i) => i * i));
  l.add(25);
  print('$l ${l.runtimeType}');
  final d = await Isolate.run(() => <double>[0.5, 1.5]);
  tentar('isolado double recebe int', () => (d as List<num>).add(1));
  print(d);
}
