// Mapas e conjuntos grandes no espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md
// §5.3): `Map<String, int>` e `Set<int>` com 200 mil entradas, remoção e
// reinserção (a ordem de inserção se mantém), iteração com modificação
// (ConcurrentModificationError capturado), mapas e conjuntos `const`
// (canonicalizados) e `Map.unmodifiable`/`Set.unmodifiable` (erro ao modificar).

void tentar(String nome, Object? Function() f) {
  try {
    print('$nome: ${f()}');
  } on ConcurrentModificationError catch (e) {
    print('$nome: ConcurrentModificationError (${e.modifiedObject.runtimeType})');
  } on UnsupportedError catch (e) {
    print('$nome: UnsupportedError ${e.message}');
  } catch (e) {
    print('$nome: ${e.runtimeType} $e');
  }
}

int resumo(Iterable<int> xs) {
  var h = 0;
  for (final x in xs) {
    h = (h * 31 + x) & 0x3FFFFFFF;
  }
  return h;
}

const mapaConst = {'um': 1, 'dois': 2, 'três': 3, 'Ā': 256};
const conjuntoConst = {1, 'x', null, true};
const mapaConst2 = {'um': 1, 'dois': 2, 'três': 3, 'Ā': 256};

void main() {
  const n = 200000;

  // Map<String, int>.
  final mapa = <String, int>{};
  for (var i = 0; i < n; i++) {
    mapa['chave-$i'] = i;
  }
  print('mapa: ${mapa.length} ${mapa['chave-0']} ${mapa['chave-199999']} ${mapa['nada']}');
  print('ordem: ${mapa.keys.take(3).toList()} ... ${mapa.keys.last}');
  for (var i = 0; i < n; i += 2) {
    mapa.remove('chave-$i');
  }
  print('após remover pares: ${mapa.length} ${mapa.keys.first} ${mapa.containsKey('chave-10')}');
  for (var i = 0; i < n; i += 4) {
    mapa['chave-$i'] = -i;
  }
  print('após reinserir: ${mapa.length} primeira=${mapa.keys.first} última=${mapa.keys.last} ${mapa['chave-8']}');
  mapa['chave-1'] = 1000; // atualizar não muda a ordem
  print('atualizado: ${mapa.keys.first} ${mapa['chave-1']}');
  var somaV = 0;
  mapa.forEach((k, v) => somaV += v);
  print('soma dos valores: $somaV resumo das chaves: ${resumo(mapa.keys.take(5000).map((k) => k.length))}');
  mapa.removeWhere((k, v) => v < 0 && v % 3 == 0);
  mapa.updateAll((k, v) => v * 2);
  print('removeWhere/updateAll: ${mapa.length} ${mapa['chave-3']} ${mapa['chave-4']}');
  final putIf = mapa.putIfAbsent('nova', () => 7);
  final putIf2 = mapa.putIfAbsent('nova', () => 8);
  print('putIfAbsent: $putIf $putIf2 ${mapa.keys.last}');
  print('update: ${mapa.update('nova', (v) => v + 1)} ${mapa.update('outra', (v) => v, ifAbsent: () => -1)}');

  // Set<int>.
  final conj = <int>{};
  for (var i = 0; i < n; i++) {
    conj.add(i * 7919 % 1000003);
  }
  print('conjunto: ${conj.length} ${conj.first} ${conj.last} ${conj.contains(7919)} ${conj.contains(1)}');
  var removidos = 0;
  for (var i = 0; i < n; i += 3) {
    if (conj.remove(i * 7919 % 1000003)) removidos++;
  }
  print('removidos: $removidos restam ${conj.length} primeiro=${conj.first}');
  for (var i = 0; i < n; i += 6) {
    conj.add(i * 7919 % 1000003);
  }
  print('reinseridos: ${conj.length} último=${conj.last} resumo=${resumo(conj.take(10000))}');
  print('add repetido: ${conj.add(7919)} ${conj.add(-1)} ${conj.last}');
  final outro = {for (var i = 0; i < 1000; i++) i * 7919 % 1000003};
  print('interseção: ${conj.intersection(outro).length} diferença: ${outro.difference(conj).length} '
      'união: ${conj.union(outro).length}');
  final grandes = <int>{};
  for (var i = 0; i < 1000; i++) {
    grandes.add(4611686018427387904 + i * 1000000007);
    grandes.add(-i);
  }
  print('ints grandes: ${grandes.length} ${grandes.contains(4611686018427387904 + 999 * 1000000007)}');
  final doubles = <double>{}..addAll([0.0, -0.0, double.nan, double.nan, 1.5, 1.5]);
  print('doubles: ${doubles.length} $doubles');

  // Iteração com modificação.
  final pequeno = {'a': 1, 'b': 2, 'c': 3};
  tentar('mapa add durante iteração', () {
    for (final k in pequeno.keys) {
      if (k == 'a') pequeno['d'] = 4;
    }
    return 'sem erro';
  });
  print('pequeno depois: $pequeno');
  tentar('mapa remove durante iteração', () {
    for (final e in pequeno.entries) {
      if (e.key == 'b') pequeno.remove('c');
    }
    return 'sem erro';
  });
  tentar('mapa atualiza valor durante iteração', () {
    for (final k in pequeno.keys) {
      pequeno[k] = 0;
    }
    return pequeno;
  });
  final cj = {1, 2, 3};
  tentar('conjunto add durante iteração', () {
    for (final x in cj) {
      if (x == 2) cj.add(10);
    }
    return 'sem erro';
  });
  tentar('conjunto grande remove durante iteração', () {
    var k = 0;
    for (final x in conj) {
      if (++k == 50000) conj.remove(x);
    }
    return 'sem erro';
  });
  tentar('lista de chaves é cópia', () {
    for (final k in pequeno.keys.toList()) {
      pequeno.remove(k);
    }
    return pequeno;
  });

  // Constantes.
  print('const: $mapaConst $conjuntoConst');
  print('const idênticos: ${identical(mapaConst, mapaConst2)} '
      '${identical(conjuntoConst, const {1, 'x', null, true})}');
  print('const lookup: ${mapaConst['três']} ${mapaConst['Ā']} ${conjuntoConst.contains(null)} ${conjuntoConst.contains(true)}');
  tentar('const map grava', () => mapaConst['um'] = 9);
  tentar('const map remove', () => mapaConst.remove('um'));
  tentar('const set add', () => conjuntoConst.add(3));
  tentar('const map clear', () {
    mapaConst.clear();
    return mapaConst;
  });

  // Map.unmodifiable e Set.unmodifiable.
  final fonte = {'x': 1, 'y': 2};
  final umod = Map<String, int>.unmodifiable(fonte);
  fonte['z'] = 3;
  print('unmodifiable é cópia: $umod ${umod.length}');
  tentar('unmodifiable grava', () => umod['x'] = 5);
  tentar('unmodifiable putIfAbsent', () => umod.putIfAbsent('w', () => 0));
  tentar('unmodifiable remove', () => umod.remove('x'));
  final umodSet = Set<int>.unmodifiable([3, 1, 2, 1]);
  print('Set.unmodifiable: $umodSet');
  tentar('Set.unmodifiable add', () => umodSet.add(4));
  final visao = Map<String, int>.from(fonte);
  visao['w'] = 0;
  print('Map.from: $visao ${fonte.length}');
  final grandeUmod = Map<String, int>.unmodifiable(mapa);
  print('unmodifiable grande: ${grandeUmod.length} ${grandeUmod['chave-3']} ${grandeUmod.keys.last}');
  tentar('unmodifiable grande remove', () => grandeUmod.remove('chave-3'));

  // Limpeza e reuso.
  mapa.clear();
  conj.clear();
  for (var i = 0; i < 1000; i++) {
    mapa['r$i'] = i;
    conj.add(-i);
  }
  print('reuso: ${mapa.length} ${conj.length} ${mapa.keys.first} ${conj.last}');
}
