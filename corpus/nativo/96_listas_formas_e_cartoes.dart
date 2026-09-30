// Listas no espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §2.7, §2.16, §5.3):
// `List<int>`/`<double>`/`<bool>` em forma compacta e a descompactação (gravar
// num `List<num>`/`List<Object?>` um valor que a forma não guarda), crescimento,
// `List.filled`/`generate`/`unmodifiable`/`of`, os erros de lista fixa e
// imutável, `sublist` e `sort`; e uma lista de 100 mil elementos criada cedo, que
// envelhece sob muita alocação e recebe objetos novos em pontos espalhados entre
// as rodadas (barreira de escrita e cartões), conferida inteira no fim.

class Caixa {
  final int v;
  final String s;
  Caixa(this.v) : s = 'c$v';
  @override
  String toString() => 'Caixa($v)';
}

void tentar(String nome, Object? Function() f) {
  try {
    print('$nome: ${f()}');
  } on UnsupportedError catch (e) {
    print('$nome: UnsupportedError ${e.message}');
  } on RangeError catch (e) {
    print('$nome: RangeError ${e.message}');
  } on TypeError catch (e) {
    print('$nome: TypeError ${e.runtimeType}');
  } catch (e) {
    print('$nome: ${e.runtimeType} $e');
  }
}

/// Alocação que só produz lixo (rodada de pressão).
int lixo(int rodada) {
  var acum = 0;
  for (var i = 0; i < 20000; i++) {
    final l = [i, i + 1.5, 'x$i', Caixa(i)];
    acum += (l[3] as Caixa).v & 7;
  }
  return acum + rodada;
}

void main() {
  // Formas compactas.
  final ints = <int>[1, 2, 3];
  final dbls = <double>[1.5, -0.0, double.infinity];
  final bools = <bool>[true, false];
  for (var i = 0; i < 100; i++) {
    ints.add(i * i);
    dbls.add(i / 4);
    bools.add(i % 3 == 0);
  }
  print('ints: ${ints.length} ${ints.runtimeType} soma=${ints.reduce((a, b) => a + b)}');
  print('dbls: ${dbls.length} ${dbls.runtimeType} ${dbls.take(4).toList()}');
  print('bools: ${bools.length} ${bools.runtimeType} verdadeiros=${bools.where((b) => b).length}');
  ints[5] = 4611686018427387904; // int fora do Smi numa List<int>
  ints[6] = -9223372036854775807 - 1;
  print('ints grandes: ${ints[5]} ${ints[6]} ${ints[5] + ints[6]}');

  // Descompactação: List<num> que recebe double depois de ints, e outros tipos.
  final nums = <num>[1, 2, 3];
  nums.add(4.5);
  nums[0] = 1e100;
  nums.add(4611686018427387904);
  print('nums: $nums ${nums.runtimeType}');
  final fixaNum = List<num>.filled(5, 0);
  fixaNum[2] = 2.5;
  fixaNum[4] = -1;
  print('fixa num: $fixaNum');
  final objs = List<Object?>.generate(6, (i) => i);
  objs[1] = 1.25;
  objs[2] = null;
  objs[3] = 'três';
  objs[4] = true;
  objs[5] = Caixa(5);
  print('objs: $objs');
  final dinamica = <dynamic>[1, 2, 3];
  dinamica.add(2.5);
  dinamica.add([1]);
  print('dynamic: $dinamica');
  final covariante = <int>[1, 2] as List<num>;
  tentar('covariante recebe double', () => covariante.add(1.5));
  tentar('covariante recebe int', () {
    covariante.add(3);
    return covariante;
  });
  final nulaveis = <int?>[1, null, 3];
  nulaveis.add(null);
  print('int?: $nulaveis ${nulaveis.whereType<int>().length}');
  final dblFixa = List<double>.filled(4, 0.0);
  dblFixa[1] = double.nan;
  dblFixa[2] = -0.0;
  print('double fixa: $dblFixa ${dblFixa[1].isNaN} ${dblFixa[2].isNegative}');

  // Crescimento.
  final cresce = <int>[];
  final capacidades = <int>[];
  for (var i = 0; i < 70000; i++) {
    cresce.add(i);
    if (i & (i - 1) == 0 && i > 0) capacidades.add(cresce.length);
  }
  print('cresce: ${cresce.length} ${cresce[69999]} marcos=${capacidades.length}');
  cresce.length = 10;
  print('encolhe: $cresce');
  final cresceNula = <String?>[];
  cresceNula.length = 3;
  print('length maior: $cresceNula');
  cresce.insertAll(5, [100, 200]);
  cresce.removeRange(0, 3);
  cresce.insert(0, -1);
  print('insere/remove: $cresce');

  // Construtores.
  final filled = List<int>.filled(3, 7);
  final filledGrow = List<int>.filled(3, 7, growable: true)..add(8);
  final gerada = List.generate(5, (i) => i * 0.5);
  final geradaFixa = List<bool>.generate(3, (i) => i.isOdd, growable: false);
  final imut = List<int>.unmodifiable([1, 2, 3]);
  final deOutra = List<double>.of(gerada);
  final deOutraFixa = List<num>.of(ints.take(3), growable: false);
  final vazia = List<String>.empty();
  final constante = const [1, 2.5, 'a'];
  print('filled: $filled $filledGrow');
  print('generate: $gerada $geradaFixa');
  print('unmodifiable: $imut of: $deOutra $deOutraFixa vazia: $vazia const: $constante');

  // Erros de lista fixa e imutável.
  tentar('fixa add', () => filled.add(1));
  tentar('fixa removeLast', () => filled.removeLast());
  tentar('fixa length=', () => filled.length = 1);
  tentar('fixa clear', () => geradaFixa.clear());
  tentar('fixa grava', () {
    filled[0] = 42;
    return filled;
  });
  tentar('imutável grava', () => imut[0] = 9);
  tentar('imutável add', () => imut.add(9));
  tentar('imutável sort', () => imut.sort());
  tentar('const grava', () => constante[0] = 0);
  tentar('const add', () => constante.add(0));
  tentar('vazia add', () => vazia.add('x'));
  tentar('índice negativo', () => ints[-1]);
  tentar('índice além', () => filled[3]);
  tentar('fixa of grava', () {
    deOutraFixa[0] = 1.5;
    return deOutraFixa;
  });
  final naoMod = List<int>.unmodifiable(cresce);
  tentar('unmodifiable de crescível', () => naoMod.removeAt(0));
  print('unmodifiable é cópia: ${naoMod.length} ${naoMod.first}');

  // sublist e sort.
  final base = List.generate(30, (i) => (i * 37) % 31);
  final sub = base.sublist(5, 15);
  sub[0] = -100;
  print('sublist: $sub base[5]=${base[5]}');
  print('sublist até o fim: ${base.sublist(25)}');
  tentar('sublist inválida', () => base.sublist(10, 5));
  base.sort();
  print('sort: $base');
  final dsort = [3.5, -0.0, 0.0, double.infinity, -1e10, 2.25];
  dsort.sort();
  print('sort double: $dsort');
  final caixas = List.generate(10, (i) => Caixa((i * 7) % 10));
  caixas.sort((a, b) => b.v.compareTo(a.v));
  print('sort caixas: $caixas');
  final strs = ['pera', 'Ā', 'abacate', 'ÿ', 'Abacaxi', ''];
  strs.sort();
  print('sort strings: $strs');
  final boolsOrd = List<bool>.of(bools.take(6))..sort((a, b) => a == b ? 0 : (a ? 1 : -1));
  print('sort bools: $boolsOrd');

  // Lista velha que recebe objetos novos (cartões).
  const n = 100000;
  final velha = List<Object?>.filled(n, null);
  for (var i = 0; i < n; i++) {
    velha[i] = i; // inteiros (sem referência)
  }
  final velhaCaixas = List<Caixa>.generate(n, (i) => Caixa(i));
  final velhaDouble = List<num>.filled(n, 0);
  var ruido = 0;
  for (var rodada = 0; rodada < 25; rodada++) {
    ruido += lixo(rodada);
    // Pontos espalhados: passo primo, deslocado por rodada.
    for (var k = 0; k < 400; k++) {
      final i = (rodada * 7919 + k * 2477) % n;
      velha[i] = Caixa(i * 3 + rodada);
      velhaCaixas[(i * 13) % n] = Caixa(((i * 13) % n) + n);
      velhaDouble[(i * 17) % n] = i + 0.5;
    }
    velha[n - 1 - rodada] = 'texto novo $rodada';
    velha[rodada * 3] = [rodada, rodada + 0.25, 'l$rodada'];
  }
  // Conferência.
  var caixasNovas = 0, textos = 0, listas = 0, inteiros = 0, erros = 0;
  for (var i = 0; i < n; i++) {
    final v = velha[i];
    if (v is int) {
      inteiros++;
      if (v != i) erros++;
    } else if (v is Caixa) {
      caixasNovas++;
      if (v.s != 'c${v.v}') erros++;
      if ((v.v - i * 3) < 0 || (v.v - i * 3) >= 25) erros++;
    } else if (v is String) {
      textos++;
      if (!v.startsWith('texto novo ')) erros++;
    } else if (v is List) {
      listas++;
      if (v[2] != 'l${v[0]}' || v[1] != (v[0] as int) + 0.25) erros++;
    } else {
      erros++;
    }
  }
  var trocadas = 0;
  for (var i = 0; i < n; i++) {
    final c = velhaCaixas[i];
    if (c.v == i) continue;
    if (c.v == i + n && c.s == 'c${i + n}') {
      trocadas++;
    } else {
      erros++;
    }
  }
  var somaD = 0.0;
  var dComMeio = 0;
  for (final d in velhaDouble) {
    somaD += d;
    if (d is double) dComMeio++;
  }
  print('velha: inteiros=$inteiros caixas=$caixasNovas textos=$textos listas=$listas');
  print('velhaCaixas trocadas=$trocadas velhaDouble doubles=$dComMeio soma=$somaD');
  print('erros: $erros ruído=$ruido');
}
