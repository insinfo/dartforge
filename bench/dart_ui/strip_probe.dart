/// Differential probe of the current dart_ui strip rasterizer and geometry.
/// Default mode emits every coverage byte and active packed record, rather
/// than treating a checksum or successful compilation as a correctness oracle.
/// `--benchmark N` times rasterization plus consumption of every active packed
/// record and alpha byte after warming all reusable buffers.
import 'dart:typed_data';

import 'package:dart_ui/src/geometry/path.dart';
import 'package:dart_ui/src/geometry/rect.dart';
import 'package:dart_ui/src/geometry/transform2d.dart';
import 'package:dart_ui/src/rendering/gpu/vector/native_strip_rasterizer.dart';
import 'package:dart_ui/src/rendering/gpu/vector/sparse_strips.dart';
import 'package:dart_ui/src/rendering/path/fill_rule.dart';

final class Scene {
  Scene(this.name, this.path, this.clip,
      {this.rule = FillRule.nonZero, this.transform = Transform2D.identity});
  final String name;
  final Path path;
  final Rect clip;
  final FillRule rule;
  final Transform2D transform;
}

void addRectangle(PathBuilder builder, double left, double top, double right,
    double bottom, bool clockwise) {
  builder.moveTo(left, top);
  if (clockwise) {
    builder.lineTo(right, top);
    builder.lineTo(right, bottom);
    builder.lineTo(left, bottom);
  } else {
    builder.lineTo(left, bottom);
    builder.lineTo(right, bottom);
    builder.lineTo(right, top);
  }
  builder.close();
}

Path rectangle(double left, double top, double right, double bottom) {
  final builder = PathBuilder();
  addRectangle(builder, left, top, right, bottom, true);
  return builder.build();
}

List<Scene> scenes() {
  const clip = Rect.fromLTRB(0, 0, 32, 32);
  final hole = PathBuilder();
  addRectangle(hole, 3.5, 3.5, 28.5, 28.5, true);
  addRectangle(hole, 10.25, 10.25, 21.75, 21.75, false);
  final overlap = PathBuilder();
  addRectangle(overlap, 3.5, 3.5, 18.5, 18.5, true);
  addRectangle(overlap, 12.5, 12.5, 27.5, 27.5, true);
  final triangle = PathBuilder();
  triangle.moveTo(3.25, 3.25);
  triangle.lineTo(26.125, 10.375);
  triangle.lineTo(10.25, 27.75);
  triangle.close();
  final curve = PathBuilder();
  curve.moveTo(4, 16);
  curve.cubicTo(4, 4, 28, 4, 28, 16);
  curve.cubicTo(28, 28, 4, 28, 4, 16);
  curve.close();
  final holePath = hole.build();
  final overlapPath = overlap.build();
  return [
    Scene('aligned', rectangle(4, 4, 20, 20), clip),
    Scene('fractional', rectangle(4.25, 3.5, 19.75, 20.125), clip),
    Scene('triangle', triangle.build(), clip),
    Scene('curve', curve.build(), clip),
    Scene('hole-nonzero', holePath, clip),
    Scene('hole-evenodd', holePath, clip, rule: FillRule.evenOdd),
    Scene('overlap-nonzero', overlapPath, clip),
    Scene('overlap-evenodd', overlapPath, clip, rule: FillRule.evenOdd),
    Scene('oversized', rectangle(-40, -40, 120, 120), clip),
    Scene('fractional-clip', rectangle(-8.5, -5.25, 20.75, 18.125),
        const Rect.fromLTRB(-3.25, -1.5, 29.2, 28.2)),
    Scene('affine', rectangle(4, 4, 20, 20), clip,
        transform: const Transform2D(1, 0.25, -0.125, 1, 2.5, -1.25)),
  ];
}

StripBuffer fill(NativeStripRasterizer rasterizer, Scene scene) => rasterizer
    .fill(scene.path, scene.clip, rule: scene.rule, transform: scene.transform);

Uint8List coverage(StripBuffer strips, Rect clip) {
  final left = clip.left.floor();
  final top = clip.top.floor();
  final width = clip.right.ceil() - left;
  final height = clip.bottom.ceil() - top;
  final pixels = Uint8List(width * height);
  for (var i = 0; i < strips.stripCount; i++) {
    final x = strips.stripX(i) - left;
    final y = strips.stripY(i) - top;
    final count = strips.stripWidth(i);
    final offset = strips.stripAlphaOffset(i);
    if (count <= 0 ||
        offset < 0 ||
        offset + count * kStripHeight > strips.alphaCount) {
      throw StateError('invalid strip alpha range');
    }
    for (var row = 0; row < kStripHeight; row++) {
      final py = y + row;
      if (py < 0 || py >= height) continue;
      for (var column = 0; column < count; column++) {
        final px = x + column;
        if (px >= 0 && px < width) {
          pixels[py * width + px] = strips.stripAlpha(i, column, row);
        }
      }
    }
  }
  for (var i = 0; i < strips.fillCount; i++) {
    final x = strips.fillX(i) - left;
    final y = strips.fillY(i) - top;
    final count = strips.fillWidth(i);
    if (count <= 0) throw StateError('invalid fill width');
    for (var row = 0; row < kStripHeight; row++) {
      final py = y + row;
      if (py < 0 || py >= height) continue;
      for (var column = 0; column < count; column++) {
        final px = x + column;
        if (px >= 0 && px < width) pixels[py * width + px] = 255;
      }
    }
  }
  return pixels;
}

String hex(Uint8List bytes, int count) {
  const digits = '0123456789abcdef';
  final output = StringBuffer();
  for (var i = 0; i < count; i++) {
    output.write(digits[bytes[i] >> 4]);
    output.write(digits[bytes[i] & 15]);
  }
  return output.toString();
}

String packed(Int32List values, int count) {
  final output = StringBuffer();
  for (var i = 0; i < count; i++) {
    if (i != 0) output.write(',');
    output.write(values[i]);
  }
  return output.toString();
}

void validate(String name, Uint8List pixels) {
  var inked = 0;
  for (final pixel in pixels) {
    if (pixel != 0) inked++;
  }
  if (inked == 0) throw StateError('$name rendered no coverage');
  if (name == 'aligned' || name == 'oversized') {
    for (var y = 0; y < 32; y++) {
      for (var x = 0; x < 32; x++) {
        final expected =
            name == 'oversized' || (x >= 4 && x < 20 && y >= 4 && y < 20)
                ? 255
                : 0;
        if (pixels[y * 32 + x] != expected)
          throw StateError('$name pixel $x,$y');
      }
    }
  }
  if (name == 'hole-nonzero' ||
      name == 'hole-evenodd' ||
      name == 'overlap-evenodd') {
    if (pixels[16 * 32 + 16] != 0) throw StateError('$name winding hole');
  }
  if (name == 'overlap-nonzero' && pixels[16 * 32 + 16] != 255) {
    throw StateError('nonzero overlap lost winding');
  }
}

/// Observes the complete output before the next fill reuses its storage.
/// The bounded integer arithmetic is identical on the VM and native AOT.
int consume(StripBuffer strips, int checksum) {
  checksum = (checksum * 31 + strips.stripCount) & 0x7fffffff;
  checksum = (checksum * 31 + strips.fillCount) & 0x7fffffff;
  checksum = (checksum * 31 + strips.alphaCount) & 0x7fffffff;
  for (var i = 0; i < strips.stripCount * kStripStride; i++) {
    checksum = (checksum * 31 + strips.strips[i]) & 0x7fffffff;
  }
  for (var i = 0; i < strips.fillCount * kFillStride; i++) {
    checksum = (checksum * 31 + strips.fills[i]) & 0x7fffffff;
  }
  for (var i = 0; i < strips.alphaCount; i++) {
    checksum = (checksum * 31 + strips.alphas[i]) & 0x7fffffff;
  }
  return checksum;
}

void main(List<String> args) {
  final fixtures = scenes();
  final rasterizer = NativeStripRasterizer();
  if (args.isNotEmpty) {
    if (args.length != 2 || args[0] != '--benchmark') {
      throw ArgumentError('usage: strip_probe.dart [--benchmark iterations]');
    }
    final iterations = int.parse(args[1]);
    if (iterations <= 0) throw ArgumentError('iterations must be positive');
    for (var round = 0; round < 20; round++) {
      for (final scene in fixtures) fill(rasterizer, scene);
    }
    final growths = rasterizer.bufferGrowths;
    var quads = 0;
    var checksum = 0;
    final watch = Stopwatch()..start();
    for (var round = 0; round < iterations; round++) {
      for (final scene in fixtures) {
        final strips = fill(rasterizer, scene);
        quads += strips.quadCount;
        checksum = consume(strips, checksum);
      }
    }
    watch.stop();
    print(
        'benchmark iterations=$iterations scenes=${fixtures.length} quads=$quads checksum=$checksum microseconds=${watch.elapsedMicroseconds} bufferGrowths=${rasterizer.bufferGrowths - growths}');
    return;
  }
  for (final scene in fixtures) {
    final strips = fill(rasterizer, scene);
    final pixels = coverage(strips, scene.clip);
    validate(scene.name, pixels);
    print(
        '${scene.name} lines=${rasterizer.lineCount} tiles=${rasterizer.tileCount} strips=${strips.stripCount} fills=${strips.fillCount} alphas=${strips.alphaCount}');
    print('strips=${packed(strips.strips, strips.stripCount * kStripStride)}');
    print('fills=${packed(strips.fills, strips.fillCount * kFillStride)}');
    print('alphas=${hex(strips.alphas, strips.alphaCount)}');
    print('pixels=${hex(pixels, pixels.length)}');
    final repeat = coverage(fill(rasterizer, scene), scene.clip);
    for (var i = 0; i < pixels.length; i++) {
      if (pixels[i] != repeat[i])
        throw StateError('${scene.name} reuse pixel $i');
    }
  }
  print('strip fixtures verified');
}
