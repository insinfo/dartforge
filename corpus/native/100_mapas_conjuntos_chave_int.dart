// Mapas e conjuntos de chave `int` pelo caminho rápido do runtime
// (docs/NATIVO-PLANO.md §13): a sonda e a gravação sem o `Heap`, a reinserção
// em lote do `_rehash` e a mistura com chaves que o runtime não conhece.
// Chaves negativas, grandes (`_Mint`), os extremos de 64 bits, padrões de hash
// que colidem, remoção no meio do crescimento, `1` diante de `1.0`, `NaN`, e a
// ordem de iteração (a de inserção) depois de tudo isso.

int soma(Iterable<int> xs) {
  var h = 0;
  for (final x in xs) {
    h = (h * 31 + (x & 0xFFFFFFF)) & 0x3FFFFFFF;
  }
  return h;
}

void extremos() {
  const mn = -9223372036854775808, mx = 9223372036854775807;
  final chaves = <int>[
    0, 1, -1, 2, -2, 7, 1 << 30, -(1 << 30), (1 << 62) - 1, -(1 << 62),
    1 << 62, -(1 << 62) - 1, mx, mn, mx - 1, mn + 1, 0x7FFFFFFF, 0x80000000,
    0xFFFFFFFF, 0x100000000, -0x100000000, 81207, 123456789012345,
  ];
  final m = <int, int>{};
  for (var i = 0; i < chaves.length; i++) {
    m[chaves[i]] = i;
  }
  print('extremos: ${m.length} ${m.keys.toList()}');
  for (final k in chaves) {
    if (m[k] != chaves.indexOf(k)) print('ERRO extremo $k: ${m[k]}');
  }
  // Uma chave `_Mint` calculada (outra caixa, o mesmo valor) acha a entrada.
  final grande = (1 << 61) * 2 + 5;
  m[grande] = -1;
  print('mint calculado: ${m[(1 << 62) + 5]} ${m.containsKey(mx - 1)} ${m.containsKey(mx - 2)}');
  print('hashCode: ${[for (final k in chaves.take(8)) k.hashCode]}');
  final c = <int>{...chaves, ...chaves.reversed};
  print('conjunto extremos: ${c.length} ${c.contains(mn)} ${c.lookup(mx)} ${c.contains(3)}');
}

void crescimentoComRemocao() {
  // Remove no meio do crescimento: as posições removidas (`_DELETED_PAIR`)
  // e as entradas removidas do `_data` passam por vários `_rehash`.
  final m = <int, int>{};
  final c = <int>{};
  for (var i = 0; i < 30000; i++) {
    final k = (i * 7919) % 50021 - 25000;
    m[k] = i;
    c.add(k);
    if (i % 3 == 2) {
      final r = ((i - 1) * 7919) % 50021 - 25000;
      m.remove(r);
      c.remove(r);
    }
    if (i % 1000 == 999) {
      // Reinsere uma removida: entra no fim da ordem.
      final r = ((i - 2) * 7919) % 50021 - 25000;
      m[r] = -i;
      c.add(r);
    }
  }
  print('crescimento: ${m.length} ${c.length} ${soma(m.keys)} ${soma(m.values)} ${soma(c)}');
  print('primeiras: ${m.keys.take(6).toList()} ultimas: ${m.keys.skip(m.length - 4).toList()}');
  var falta = 0;
  for (var i = -25000; i < 25021; i++) {
    if (m.containsKey(i) != c.contains(i)) falta++;
  }
  print('consistencia: $falta');
}

void colisoes() {
  // Chaves que caem na mesma primeira sonda de uma tabela pequena: múltiplos
  // de uma potência de dois grande, e as que diferem só nos bits altos.
  final m = <int, String>{};
  for (var i = 0; i < 64; i++) {
    m[i << 40] = 'a$i';
    m[(i << 40) + 1] = 'b$i';
    m[-(i << 52)] = 'c$i';
  }
  var ok = true;
  for (var i = 0; i < 64; i++) {
    ok &= m[i << 40] == (i == 0 ? 'c0' : 'a$i') || (i == 0 && m[0] == 'c0');
    ok &= m[(i << 40) + 1] == 'b$i';
  }
  print('colisoes: ${m.length} $ok ${m.keys.take(5).toList()}');
}

void numeros() {
  // `1` e `1.0` são a mesma chave (`==` de `num`, que o runtime deixa para o
  // Dart); `NaN` nunca é igual a si mesmo.
  final m = <num, String>{1: 'um', 2.5: 'dois e meio'};
  m[1.0] = 'um de novo';
  m[2] = 'dois';
  m[double.nan] = 'nan1';
  m[double.nan] = 'nan2';
  m[-0.0] = 'menos zero';
  m[0] = 'zero';
  print('numeros: ${m.length} ${m[1]} ${m[1.0]} ${m.keys.toList()} ${m[0]} ${m[-0.0]}');
  final c = <num>{1, 1.0, 2, 2.0, 3.5, double.nan, double.nan};
  print('conjunto num: ${c.length} ${c.contains(2.0)} ${c.lookup(1.0)} ${c.lookup(1.0).runtimeType}');
  final o = <Object?, int>{null: 0, 'x': 1, 1: 2, true: 3, 1.5: 4, (1, 2): 5};
  o[1] = 20;
  o['x'] = 10;
  o[null] = -1;
  print('mistos: $o ${o[(1, 2)]} ${o.containsKey(false)}');
}

void ordemEOperacoes() {
  final m = <int, int>{};
  for (var i = 20; i > 0; i--) {
    m[i * i - 100] = i;
  }
  m.remove(0);
  m[0] = 99;
  m.update(-99, (v) => v * 1000);
  m.putIfAbsent(-100, () => 7);
  m.putIfAbsent(1000, () => 8);
  print('ordem: ${m.keys.toList()}');
  print('valores: ${m.values.toList()}');
  final copia = Map<int, int>.of(m)..addAll({1: 1, 2: 2, 300: 3});
  final ids = Map<int, int>.identity()..addAll(m);
  print('copia: ${copia.length} ${ids.length} ${ids[300]} ${copia[300]} $copia');
  m.clear();
  m[5] = 5;
  print('depois de clear: $m ${m.length}');
  final conta = <int, int>{};
  for (var i = 0; i < 5000; i++) {
    final k = (i * i) % 97 - 48;
    conta[k] = (conta[k] ?? 0) + 1;
  }
  print('conta: ${conta.length} ${soma(conta.keys)} ${soma(conta.values)}');
  final s = <int>{};
  for (var i = 0; i < 3000; i++) {
    s.add(-i * 3);
  }
  s.removeWhere((x) => x % 2 == 0);
  s.addAll([for (var i = 0; i < 100; i++) i]);
  print('conjunto: ${s.length} ${s.first} ${s.last} ${soma(s)} ${s.contains(-3)} ${s.contains(-6)}');
}

void grande() {
  final m = <int, int>{};
  for (var i = 0; i < 200000; i++) {
    m[i * 31 % 200000] = i;
  }
  var s = 0;
  for (var i = 0; i < 200000; i++) {
    s += m[i] ?? 0;
  }
  final c = <int>{for (var i = 0; i < 100000; i++) i * 1000003};
  print('grande: ${m.length} $s ${c.length} ${c.contains(99999 * 1000003)} ${soma(c.take(10))}');
}

void main() {
  extremos();
  crescimentoComRemocao();
  colisoes();
  numeros();
  ordemEOperacoes();
  grande();
}
