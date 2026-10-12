// Gravação SIMD sem caixa e leitura de campo `late` sem chamada ao runtime
// (docs/SIMD-NATIVO.md, laços do rasterizador do dgfx): `lista[i] = v` numa
// `Int32x4List`/`Float32x4List`/`Float64x2List` grava o vetor direto e só
// encaixota no caminho lento (índice fora, visão não modificável); o campo
// `late` de tipo não anulável sai direto quando já tem valor, e os casos que
// dependem do estado "não inicializado" (erro, inicializador que falha,
// ciclo, `late` anulável com `null`) continuam iguais aos da VM.
import 'dart:typed_data';

String i4(Int32x4 v) => '[${v.x}, ${v.y}, ${v.z}, ${v.w}]';
String f4(Float32x4 v) => '[${v.x}, ${v.y}, ${v.z}, ${v.w}]';

int contador = 0;

int calcular() {
  contador++;
  return contador * 10;
}

int falhas = 0;

Int32List talvez() {
  falhas++;
  if (falhas == 1) throw StateError('primeira');
  return Int32List(2)..[0] = falhas;
}

class Campos {
  late Int32List dados;
  late final Int32List fixa;
  late final Int32List preguicosa = Int32List.fromList([calcular(), 2, 3]);
  late Int32List? anulavel;
  late final Int32List tenta = talvez();
  late final int ciclo = ciclo + 1;
  late Int32x4List vetores;

  int somar() {
    var s = 0;
    for (var i = 0; i < dados.length; i++) {
      s += dados[i] + preguicosa[i % 3];
    }
    return s;
  }
}

void main() {
  // Gravação no laço: o valor é uma expressão SIMD.
  final a = Int32x4List(8), b = Int32x4List(8), c = Int32x4List(8);
  for (var i = 0; i < 8; i++) {
    a[i] = Int32x4(i, -i, i * 3, 0x7fffffff);
    b[i] = Int32x4(1, 2, 3, 1);
  }
  for (var i = 0; i < 8; i++) {
    c[i] = (a[i] + b[i]) & Int32x4(-1, 0xff, -1, -1);
  }
  print('c0 ${i4(c[0])} c7 ${i4(c[7])}');

  // O valor da atribuição é o vetor gravado.
  final r = (c[1] = Int32x4(5, 6, 7, 8));
  print('r ${i4(r)} ${i4(c[1])}');

  // Composto (continua pelo caminho do operador).
  c[2] += Int32x4(1, 1, 1, 1);
  print('c2 ${i4(c[2])}');

  // Float32x4List e Float64x2List.
  final f = Float32x4List(4);
  final g = Float64x2List(2);
  for (var i = 0; i < 4; i++) {
    f[i] = Float32x4(0.1, 1e38, -0.0, i.toDouble()) * Float32x4.splat(10.0);
  }
  g[1] = Float64x2(1.5, -2.5) + Float64x2.splat(0.25);
  print('f3 ${f4(f[3])} g1 [${g[1].x}, ${g[1].y}]');

  // Visão com deslocamento sobre o mesmo buffer.
  final bytes = Int32List(12);
  final visao = bytes.buffer.asInt32x4List(16, 2);
  visao[1] = Int32x4(9, 8, 7, 6);
  print('bytes $bytes');

  // Caminho lento: índice fora e visão não modificável.
  try {
    c[8] = Int32x4(1, 2, 3, 4);
    print('sem erro');
  } catch (e) {
    print('fora: ${e is RangeError}');
  }
  try {
    c[-1] = Int32x4(1, 2, 3, 4);
    print('sem erro');
  } catch (e) {
    print('negativo: ${e is RangeError}');
  }
  final so = c.asUnmodifiableView();
  try {
    so[0] = Int32x4(1, 2, 3, 4);
    print('sem erro');
  } catch (e) {
    print('nao modificavel: ${e is UnsupportedError} ${i4(so[0])}');
  }

  // `late` sem inicializador.
  final o = Campos();
  try {
    print(o.dados.length);
  } catch (e) {
    print('dados: $e');
  }
  o.dados = Int32List.fromList([1, 2, 3, 4, 5]);
  print('somar ${o.somar()} ${o.somar()} contador $contador');
  try {
    print(o.fixa.length);
  } catch (e) {
    print('fixa: $e');
  }
  o.fixa = Int32List(3);
  print('fixa ${o.fixa.length}');
  try {
    o.fixa = Int32List(1);
  } catch (e) {
    print('fixa de novo: $e');
  }

  // `late` anulável: `null` é valor inicializado.
  try {
    print(o.anulavel);
  } catch (e) {
    print('anulavel: $e');
  }
  o.anulavel = null;
  print('anulavel ${o.anulavel}');
  o.anulavel = Int32List(1);
  print('anulavel ${o.anulavel}');

  // Inicializador que falha: a próxima leitura tenta de novo.
  try {
    print(o.tenta);
  } catch (e) {
    print('tenta: $e');
  }
  print('tenta ${o.tenta} $falhas');
  print('tenta ${o.tenta} $falhas');

  // Ciclo no inicializador.
  try {
    print(o.ciclo);
  } catch (e) {
    print('ciclo: ${e.runtimeType}');
  }

  // `fillRange` de lista tipada: o runtime preenche de uma vez; o resto
  // (faixa inválida, visão não modificável, clamped) é o do SDK.
  final u32 = Uint32List(6)..fillRange(1, 5, 0xFFFFFFFF);
  final i8 = Int8List(5)..fillRange(0, 5, 300);
  final i16 = Int16List(4)..fillRange(2, 4, -70000);
  final i64 = Int64List(3)..fillRange(0, 2, -1);
  final f32 = Float32List(4)..fillRange(1, 3, 0.1);
  final f64 = Float64List(3)..fillRange(0, 3, -0.0);
  final cl = Uint8ClampedList(4)..fillRange(0, 4, 999);
  print('fill $u32 $i8 $i16 $i64 $f32 $f64 $cl');
  final base = Int32List(8);
  base.buffer.asInt32List(8, 4).fillRange(1, 3, 7);
  print('fill visao $base');
  base.fillRange(3, 3, 9);
  print('fill vazio $base');
  for (final (ini, fim) in [(-1, 2), (2, 1), (0, 9)]) {
    try {
      base.fillRange(ini, fim, 1);
      print('sem erro');
    } catch (e) {
      print('fill $ini $fim: ${e is RangeError}');
    }
  }
  try {
    base.asUnmodifiableView().fillRange(0, 1, 5);
    print('sem erro');
  } catch (e) {
    print('fill nao modificavel: ${e is UnsupportedError}');
  }
  dynamic din = Int32List(3);
  din.fillRange(0, 3, 4);
  print('fill dynamic $din');

  // Local SIMD gravado por uma função local direta (pelo endereço do local
  // de quem chama: 16 bytes, não 8).
  var acumulado = Int32x4(1, 2, 3, 4);
  var escala = Float32x4(0.5, 1.5, -2.0, 3.0);
  void somar() {
    acumulado = acumulado + acumulado;
    escala = escala * escala;
  }

  somar();
  somar();
  print('local direto ${i4(acumulado)} ${f4(escala)}');

  // Campo `late` SIMD lido no laço.
  o.vetores = Int32x4List(4);
  for (var i = 0; i < 4; i++) {
    o.vetores[i] = o.vetores[i] + Int32x4(i, i, i, i);
  }
  print('vetores ${i4(o.vetores[3])}');
}
