// The solid-color blend of the dgfx rasterizer, `(src * a + dst * (255 - a)) >> 8`
// per channel, in three forms with the same output:
//
// * scalar, over `Int32List`;
// * SIMD with the Dart 3.6 API: `Int32x4` does not multiply, so the lanes go
//   to `Float32x4` and back (the path the rasterizer uses today);
// * SIMD with Dart 3.14's `Int32x4` API (`*` and `>>`), through the
//   `dartforge_simd` package (plain Dart on the 3.6.2 VM; `<4 x i32>` in DartForge).
//
// Prints the best time per call (µs) of each form over several rounds and the
// checksum of each output (equal to each other and across compilers).
import 'dart:typed_data';

import 'package:dartforge_simd/int32x4_3_14.dart';

const channels = 1 << 16;
const callsPerRound = 400;
const rounds = 7;
const alpha = 173;

void blendScalar(Int32List src, Int32List dst, Int32List out, int a) {
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

double measure(void Function() body) {
  var best = double.infinity;
  for (var r = 0; r < rounds; r++) {
    final sw = Stopwatch()..start();
    for (var c = 0; c < callsPerRound; c++) {
      body();
    }
    final us = sw.elapsedMicroseconds / callsPerRound;
    if (us < best) best = us;
  }
  return best;
}

void main() {
  final src = Int32List(channels);
  final dst = Int32List(channels);
  var seed = 12345;
  for (var i = 0; i < channels; i++) {
    seed = (seed * 1103515245 + 12345) & 0x7fffffff;
    src[i] = seed & 0xff;
    dst[i] = (seed >> 8) & 0xff;
  }
  final out = Int32List(channels);
  final src4 = src.buffer.asInt32x4List();
  final dst4 = dst.buffer.asInt32x4List();
  final out4 = out.buffer.asInt32x4List();

  final forms = <String, void Function()>{
    'scalar': () => blendScalar(src, dst, out, alpha),
    'simd_api_3_6': () => blendSimd36(src4, dst4, out4, alpha),
    'simd_api_3_14': () => blendSimd314(src4, dst4, out4, alpha),
  };
  for (final MapEntry(key: name, value: body) in forms.entries) {
    out.fillRange(0, out.length, 0);
    body();
    final sum = checksum(out);
    final us = measure(body);
    print('$name ${us.toStringAsFixed(1)} us checksum $sum');
  }
}
