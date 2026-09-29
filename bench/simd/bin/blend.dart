// O blend de cor sólida do rasterizador do dgfx, `(src * a + dst * (255 - a)) >> 8`
// por canal, em três formas com a mesma saída:
//
// * escalar, sobre `Int32List`;
// * SIMD com a API do Dart 3.6: `Int32x4` não multiplica, então as pistas vão
//   para `Float32x4` e voltam (o caminho que o rasterizador usa hoje);
// * SIMD com a API de `Int32x4` do Dart 3.14 (`*` e `>>`), pelo pacote
//   `dartforge_simd` (na VM 3.6.2 é Dart puro; no DartForge, `<4 x i32>`).
//
// Imprime o menor tempo por chamada (µs) de cada forma em várias rodadas e o
// checksum de cada saída (iguais entre si e entre compiladores).
import 'dart:typed_data';

import 'package:dartforge_simd/int32x4_3_14.dart';

const canais = 1 << 16;
const chamadasPorRodada = 400;
const rodadas = 7;
const alfa = 173;

void blendEscalar(Int32List src, Int32List dst, Int32List out, int a) {
  final inv = 255 - a;
  for (var i = 0; i < out.length; i++) {
    out[i] = (src[i] * a + dst[i] * inv) >> 8;
  }
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

void blendSimd314(Int32x4List src, Int32x4List dst, Int32x4List out, int a) {
  final av = Int32x4(a, a, a, a);
  final iv = Int32x4(255 - a, 255 - a, 255 - a, 255 - a);
  for (var i = 0; i < out.length; i++) {
    out[i] = (src[i] * av + dst[i] * iv) >> 8;
  }
}

int checksum(Int32List v) {
  var h = 0;
  for (var i = 0; i < v.length; i++) {
    h = (h * 31 + v[i]) & 0x3fffffff;
  }
  return h;
}

double medir(void Function() corpo) {
  var melhor = double.infinity;
  for (var r = 0; r < rodadas; r++) {
    final sw = Stopwatch()..start();
    for (var c = 0; c < chamadasPorRodada; c++) {
      corpo();
    }
    final us = sw.elapsedMicroseconds / chamadasPorRodada;
    if (us < melhor) melhor = us;
  }
  return melhor;
}

void main() {
  final src = Int32List(canais);
  final dst = Int32List(canais);
  var semente = 12345;
  for (var i = 0; i < canais; i++) {
    semente = (semente * 1103515245 + 12345) & 0x7fffffff;
    src[i] = semente & 0xff;
    dst[i] = (semente >> 8) & 0xff;
  }
  final out = Int32List(canais);
  final src4 = src.buffer.asInt32x4List();
  final dst4 = dst.buffer.asInt32x4List();
  final out4 = out.buffer.asInt32x4List();

  final formas = <String, void Function()>{
    'escalar': () => blendEscalar(src, dst, out, alfa),
    'simd_api_3_6': () => blendSimd36(src4, dst4, out4, alfa),
    'simd_api_3_14': () => blendSimd314(src4, dst4, out4, alfa),
  };
  for (final MapEntry(key: nome, value: corpo) in formas.entries) {
    out.fillRange(0, out.length, 0);
    corpo();
    final soma = checksum(out);
    final us = medir(corpo);
    print('$nome ${us.toStringAsFixed(1)} us checksum $soma');
  }
}
