// Listas do núcleo pelo cabeçalho fixo (comprimento e dados lidos em linha),
// `List.filled` preenchido no runtime e `add` de escalar sem caixa: os
// mesmos resultados e erros da VM nos casos de borda.
import 'dart:collection';
import 'dart:typed_data';

class Minha extends ListBase<int> {
  final _l = <int>[];
  int adds = 0;
  @override
  int get length => _l.length;
  @override
  set length(int n) => _l.length = n;
  @override
  int operator [](int i) => _l[i] * 10;
  @override
  void operator []=(int i, int v) => _l[i] = v;
  @override
  void add(int v) {
    adds++;
    _l.add(v);
  }
}

void erro(String n, void Function() f) {
  try {
    f();
  } catch (e) {
    print('$n ${e.runtimeType} $e');
  }
}

String tentar(void Function() f) {
  try {
    f();
    return 'ok';
  } catch (e) {
    return e.runtimeType.toString();
  }
}

int soma(List<int> l) {
  var s = 0;
  for (var i = 0; i < l.length; i++) {
    s += l[i];
  }
  return s;
}

void main() {
  // add em laço: o cabeçalho acompanha o crescimento do vetor.
  final l = <int>[];
  for (var i = 0; i < 1000; i++) {
    l.add(i * 3 - 7);
    if (l[i] != i * 3 - 7) print('errado em $i');
  }
  print('${l.length} ${soma(l)} ${l.first} ${l.last}');

  // Extremos de int, double e bool sem caixa.
  final ints = <int>[]..add(-9223372036854775808)..add(9223372036854775807)..add(1 << 62)..add(-(1 << 62) - 1);
  print(ints);
  final ds = <double>[];
  ds.add(-0.0);
  ds.add(double.nan);
  ds.add(double.infinity);
  ds.add(1.5);
  print('${ds[0].isNegative} ${ds[1].isNaN} ${ds[2]} ${ds[3]}');
  final bs = <bool>[];
  bs.add(true);
  bs.add(false);
  print(bs);

  // Tamanho fixo, não modificável e covariância: os erros do SDK.
  final fixa = List<int>.filled(3, 7);
  print('${fixa} ${tentar(() => fixa.add(1))}');
  final imut = List<int>.unmodifiable([1, 2]);
  print(tentar(() => imut.add(3)));
  print(tentar(() => imut[0] = 9));
  List<int> nunca = List<Never>.empty(growable: true);
  print(tentar(() => nunca.add(1)));
  List<num> nums = <int>[1];
  print(tentar(() => nums.add(2.5)));
  print(nums);

  // Uma classe do usuário que implementa List<int>: o `add` dela.
  final m = Minha();
  m.add(4);
  m.add(5);
  print('${m.adds} ${m[0]} ${m[1]} ${soma(m)}');

  // filled: crescível, anulável, com null, bool e double.
  final g = List<int>.filled(2, 5, growable: true);
  g.add(6);
  print(g);
  final n = List<int?>.filled(3, null);
  print(n);
  final crivo = List<bool>.filled(10, true);
  for (var j = 4; j < 10; j += 2) {
    crivo[j] = false;
  }
  print(crivo);
  print(List<double>.filled(2, 0.5));
  print(List<String>.filled(2, 'x'));

  // Mudanças de estrutura entre leituras: o cabeçalho segue o vetor.
  final e = <int>[1, 2, 3, 4, 5];
  var acc = 0;
  for (var i = 0; i < e.length; i++) {
    acc += e[i];
    if (i == 1) e.insert(0, 100);
    if (i == 3) e.removeLast();
  }
  print('$acc $e');
  e.length = 2;
  print('${e.length} ${soma(e)}');
  e.clear();
  e.add(9);
  print(e);
  final c = List<int>.of([3, 1, 2]);
  c.add(0);
  c.sort();
  print(c);
  print(tentar(() => c[10]));
  print(tentar(() => c[-1] = 1));

  // A conferência de limites: `RangeError.range` com "length" na leitura e
  // "index" no `[]=`, como a VM.
  final vazia = <int>[];
  erro('vazia', () => vazia[0]);
  erro('neg', () => [1, 2][-1]);
  erro('set', () {
    final l = [1];
    l[5] = 2;
  });
  erro('fixa', () => List<int>.filled(2, 0)[2]);
  erro('const', () => const [1, 2][2]);
  erro('u8', () => Uint8List(2)[3]);
  erro('str', () => 'ab'[5]);
  erro('cu', () => 'ab'.codeUnitAt(5));
  erro('setfixa', () {
    final l = List<int>.filled(2, 0);
    l[7] = 1;
  });
  erro('setdyn', () {
    dynamic l = [1];
    l[3] = 1;
  });
  erro('elementAt', () => [1].elementAt(3));
}
