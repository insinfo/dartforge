// `sort` com comparador e as cópias de lista (`List.of`, `toList`,
// `List.from`) com os extremos de 64 bits. O `compareTo` de `int` com
// argumento `int` é calculado em linha no nativo, e as cópias entre listas do
// runtime passam os elementos sem caixa (e as compactas, bits direto): o
// resultado, os tipos reificados e os erros têm de ser os da VM.

const mn = -9223372036854775808;
const mx = 9223372036854775807;

List<int> dados(int n) {
  final l = <int>[];
  var x = 88172645463325252;
  for (var i = 0; i < n; i++) {
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    switch (i % 7) {
      case 0:
        l.add(x);
      case 1:
        l.add(x % 1000 - 500);
      case 2:
        l.add((1 << 62) + (x % 3) - 1);
      case 3:
        l.add(i.isEven ? mn : mx);
      case 4:
        l.add(-(1 << 62) - (x % 3));
      default:
        l.add(i);
    }
  }
  return l;
}

bool ordenada(List<int> l, bool crescente) {
  for (var i = 1; i < l.length; i++) {
    final c = l[i - 1].compareTo(l[i]);
    if (crescente ? c > 0 : c < 0) return false;
  }
  return true;
}

int resumo(List<int> l) {
  var h = 17;
  for (final v in l) {
    h = (h * 31) ^ v;
  }
  return h;
}

void tentar(String nome, void Function() f) {
  try {
    f();
    print('$nome: ok');
  } on TypeError catch (e) {
    print('$nome: TypeError $e');
  } catch (e) {
    print('$nome: ${e.runtimeType} $e');
  }
}

void compareToInt() {
  final valores = <int>[mn, mn + 1, -(1 << 62) - 1, -(1 << 62), -1, 0, 1, (1 << 62) - 1, 1 << 62, mx - 1, mx];
  final linhas = <String>[];
  for (final a in valores) {
    final r = <int>[];
    for (final b in valores) {
      r.add(a.compareTo(b));
    }
    linhas.add(r.join(','));
  }
  print(linhas.join(' | '));
  // Pelo `num` e pelo `dynamic` (o membro do SDK, não o em linha).
  num n = mx;
  dynamic d = mn;
  print('${n.compareTo(mn)} ${d.compareTo(mx)} ${(mn as Comparable).compareTo(mn)}');
  // `int` com `double`: NaN, infinitos, -0.0, além de 2^53.
  const doubles = <double>[double.nan, double.infinity, double.negativeInfinity, -0.0, 0.0, 9007199254740993.0, 9223372036854775807.0, -9223372036854775808.0, 1.5];
  for (final a in <int>[0, 9007199254740993, mx, mn, 1]) {
    print('$a: ${doubles.map((b) => a.compareTo(b)).join(',')}');
  }
  print(Comparable.compare(mx, mn));
}

void ordenarComComparador() {
  final base = dados(3000);
  final cresc = List<int>.of(base)..sort((a, b) => a.compareTo(b));
  final decr = List<int>.of(base)..sort((a, b) => b.compareTo(a));
  print('crescente ${ordenada(cresc, true)} ${cresc.first} ${cresc.last} ${resumo(cresc)}');
  print('decrescente ${ordenada(decr, false)} ${decr.first} ${decr.last} ${resumo(decr)}');
  final natural = List<int>.of(base)..sort();
  print('igual ao sort() ${resumo(natural) == resumo(cresc)}');
  // Comparador que não é o compareTo: pelo valor absoluto (com o `abs` de
  // `mn`, que estoura), depois pelo sinal.
  final abs = List<int>.of(base.take(200).where((v) => v != mx && v != mn + 1))
    ..sort((a, b) {
      final c = a.abs().compareTo(b.abs());
      return c != 0 ? c : a.sign - b.sign;
    });
  print('abs ${abs.take(3).join(',')} ${abs.skip(40).take(6).join(',')} ${abs.last}');
  // Lista fixa e sublista.
  final fixa = List<int>.of(base.take(64), growable: false)..sort((a, b) => a.compareTo(b));
  print('fixa ${fixa.first} ${fixa.last} ${resumo(fixa)}');
  // `List<num>` misturada, pelo compareTo de `num` (o do SDK).
  final nums = <num>[3, 1.5, mx, -0.0, 0, mn, 2.5, -1, 0.0, 1 << 62, double.infinity, double.negativeInfinity];
  nums.sort((a, b) => a.compareTo(b));
  print('num $nums');
  // Exceção dentro do comparador.
  tentar('comparador que lança', () {
    List<int>.of(base.take(100)).sort((a, b) => a == mx ? throw StateError('mx') : a.compareTo(b));
  });
  // Comparador que devolve `dynamic`/`int` de outra forma.
  final porTexto = <int>[10, 9, mx, mn, 100, -5]..sort((a, b) => '$a'.compareTo('$b'));
  print('texto $porTexto');
}

void copias() {
  final base = <int>[mn, -1, 0, 1, mx, 1 << 62, -(1 << 62) - 1, 42];
  final fixa = List<int>.of(base, growable: false);
  final constante = const <int>[mn, mx, 0];
  final copias = <String, List<Object?>>{
    'of': List<int>.of(base),
    'of fixa': List<int>.of(base, growable: false),
    'of da fixa': List<int>.of(fixa),
    'of da constante': List<int>.of(constante),
    'toList': base.toList(),
    'toList fixa': base.toList(growable: false),
    'toList da fixa': fixa.toList(),
    'toList da constante': constante.toList(),
    'from': List<int>.from(base),
    'num.of': List<num>.of(base),
    'Object?.of': List<Object?>.of(base),
    'int?.of': List<int?>.of(base),
    'sublist': base.sublist(2, 6),
    'toList do sublist': base.sublist(1).toList(growable: false),
  };
  copias.forEach((nome, l) {
    print('$nome: ${l.runtimeType} ${l.length} $l ${l is List<int>} ${l is List<num>}');
  });
  // As cópias são independentes da origem.
  final c = List<int>.of(base);
  c[0] = 7;
  base[1] = 8;
  print('independentes ${c[0]} ${c[1]} ${base[0]} ${base[1]}');
  // O `E` da cópia é o do construtor, não o da origem.
  final numeros = List<num>.of(base)..add(1.5);
  print('num aceita double: $numeros');
  final objetos = List<Object?>.of(base)..add('x')..add(null);
  print('Object? aceita texto: $objetos');
  print('int.from de dynamic ${List<int>.from(<dynamic>[mn, mx])}');
  print('int?.of com null ${List<int?>.of(<int?>[mn, null, mx])}');
  tentar('add de texto pela cópia', () => (List<int>.of(base) as List<dynamic>).add('x'));
  tentar('[]= de double pela cópia fixa', () => (List<int>.of(base, growable: false) as List<dynamic>)[0] = 1.5);
  tentar('fixa não cresce', () => List<int>.of(base, growable: false).add(1));
  tentar('constante não muda', () => constante.toList()..add(1));
  tentar('toList da constante é modificável', () => (constante.toList()..[0] = 5).length);
  // Vazias.
  print('vazias ${List<int>.of(<int>[])} ${<int>[].toList(growable: false)} ${List<int>.of(const <int>[])}');
  // Cópia grande com o coletor: os extremos continuam iguais.
  final grande = dados(20000);
  final g = List<int>.of(grande);
  final h = g.toList(growable: false);
  var iguais = true;
  for (var i = 0; i < grande.length; i++) {
    if (g[i] != grande[i] || h[i] != grande[i]) iguais = false;
  }
  print('grande $iguais ${resumo(h)} ${h.length}');
}

void main() {
  compareToInt();
  ordenarComComparador();
  copias();
}
