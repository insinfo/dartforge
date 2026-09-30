// A ida e volta pista a pista entre `Int32x4` e `Float32x4` como conversão
// vetorial (`lower/simd.rs`, `conversao_de_pistas`):
// `Float32x4(v.x.toDouble(), …)` vira `sitofp`, e `Int32x4(r.x.toInt()
// [>> c], …)` vira `fptosi` (+ `ashr`) quando todas as pistas estão em
// `[-2^31, 2^31)` — senão, o caminho pista a pista, com o `UnsupportedError`
// do `toInt()` de NaN e infinito e a saturação e o truncamento da VM fora da
// faixa. Também as formas que não casam (pistas fora de ordem, origens
// diferentes, deslocamentos diferentes) e o blend do rasterizador com a API
// do Dart 3.6. A saída tem de ser a da VM.
import 'dart:typed_data';

Float32x4 paraFloat(Int32x4 v) {
  final f = Float32x4(v.x.toDouble(), v.y.toDouble(), v.z.toDouble(), v.w.toDouble());
  return f;
}

Int32x4 paraInt(Float32x4 r) {
  final i = Int32x4(r.x.toInt(), r.y.toInt(), r.z.toInt(), r.w.toInt());
  return i;
}

Int32x4 paraIntDesloca(Float32x4 r) {
  final i = Int32x4(r.x.toInt() >> 8, r.y.toInt() >> 8, r.z.toInt() >> 8, r.w.toInt() >> 8);
  return i;
}

Int32x4 paraIntDesloca31(Float32x4 r) {
  final i = Int32x4((r.x.toInt() >> 31), (r.y.toInt() >> 31), (r.z.toInt() >> 31), (r.w.toInt() >> 31));
  return i;
}

// Não casam: a forma de sempre.
Int32x4 foraDeOrdem(Float32x4 r) {
  final i = Int32x4(r.y.toInt(), r.x.toInt(), r.z.toInt(), r.w.toInt());
  return i;
}

Int32x4 deslocamentosDiferentes(Float32x4 r) {
  final i = Int32x4(r.x.toInt() >> 1, r.y.toInt() >> 2, r.z.toInt() >> 1, r.w.toInt() >> 1);
  return i;
}

Float32x4 origensDiferentes(Int32x4 a, Int32x4 b) {
  final f = Float32x4(a.x.toDouble(), b.y.toDouble(), a.z.toDouble(), a.w.toDouble());
  return f;
}

void blendSimd36(Int32x4List src, Int32x4List dst, Int32x4List out, int a) {
  final av = Float32x4.splat(a.toDouble());
  final iv = Float32x4.splat((255 - a).toDouble());
  for (var i = 0; i < out.length; i++) {
    final s = src[i];
    final d = dst[i];
    final sf = Float32x4(s.x.toDouble(), s.y.toDouble(), s.z.toDouble(), s.w.toDouble());
    final df = Float32x4(d.x.toDouble(), d.y.toDouble(), d.z.toDouble(), d.w.toDouble());
    final r = sf * av + df * iv;
    out[i] = Int32x4(r.x.toInt() >> 8, r.y.toInt() >> 8, r.z.toInt() >> 8, r.w.toInt() >> 8);
  }
}

String f4(Float32x4 f) => '[${f.x}, ${f.y}, ${f.z}, ${f.w}]';
String i4(Int32x4 i) => '[${i.x}, ${i.y}, ${i.z}, ${i.w}]';

void tentar(String nome, String Function() f) {
  try {
    print('$nome: ${f()}');
  } catch (e) {
    print('$nome: ${e.runtimeType} $e');
  }
}

void main() {
  for (final v in [
    Int32x4(0, 1, -1, 7),
    Int32x4(0x7fffffff, -0x80000000, 16777217, -16777217),
    Int32x4(16777219, 0x7fffff80, 0x7fffffbf, -0x7fffffc1),
  ]) {
    print('paraFloat ${i4(v)} -> ${f4(paraFloat(v))}');
  }
  print('origens ${f4(origensDiferentes(Int32x4(1, 2, 3, 4), Int32x4(5, 6, 7, 8)))}');

  final casos = <Float32x4>[
    Float32x4(0.0, -0.0, 1.5, -1.5),
    Float32x4(2.9, -2.9, 0.49, -0.99),
    Float32x4(2147483520.0, -2147483648.0, 1e-30, -1e-30),
    Float32x4(2147483648.0, 1.0, 2.0, 3.0),
    Float32x4(-2147483904.0, 1.0, 2.0, 3.0),
    Float32x4(1e20, -1e20, 3e9, -3e9),
    Float32x4(3.4028235e38, -3.4028235e38, 0.0, 0.0),
    Float32x4(double.nan, 1.0, 2.0, 3.0),
    Float32x4(1.0, 2.0, 3.0, double.infinity),
    Float32x4(1.0, double.negativeInfinity, 3.0, 4.0),
    Float32x4(65535.9, -65536.1, 255.0 * 173 + 255.0 * 82, 8388608.0),
  ];
  for (final r in casos) {
    tentar('paraInt ${f4(r)}', () => i4(paraInt(r)));
    tentar('  >> 8', () => i4(paraIntDesloca(r)));
    tentar('  >> 31', () => i4(paraIntDesloca31(r)));
    tentar('  fora de ordem', () => i4(foraDeOrdem(r)));
    tentar('  deslocamentos', () => i4(deslocamentosDiferentes(r)));
  }

  const n = 64;
  final src = Int32List(n * 4), dst = Int32List(n * 4);
  var semente = 12345;
  for (var i = 0; i < n * 4; i++) {
    semente = (semente * 1103515245 + 12345) & 0x7fffffff;
    src[i] = semente & 0xff;
    dst[i] = (semente >> 8) & 0xff;
  }
  final out = Int32List(n * 4);
  blendSimd36(src.buffer.asInt32x4List(), dst.buffer.asInt32x4List(), out.buffer.asInt32x4List(), 173);
  var h = 0;
  for (var i = 0; i < out.length; i++) {
    h = (h * 31 + out[i]) & 0x3fffffff;
  }
  print('blend36 $h ${out.sublist(0, 8)}');
  // Pistas fora da faixa de 32 bits no meio das voltas: essas voltas vão
  // pelo caminho pista a pista (o `toInt()` exato, o `>> 8` em 64 bits e o
  // truncamento do construtor).
  final g = Int32List.fromList([1, 2, 3, 4, 0x7fffffff, -0x80000000, 0x40000000, 5, 9, 8, 7, 6]);
  final out2 = Int32List(12);
  blendSimd36(g.buffer.asInt32x4List(), g.buffer.asInt32x4List(), out2.buffer.asInt32x4List(), 173);
  print('blend36 fora da faixa $out2');
}
