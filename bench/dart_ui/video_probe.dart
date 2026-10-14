// Runs the current dart_ui converter directly; no copied conversion kernel.
// Exact bytes compare backends and plane layouts. Fixed-point RGB is allowed
// one level against the framework's double reference, matching its tests.
import 'dart:typed_data';
import 'package:dart_ui/src/graphics/image/decoded_image.dart';
import 'package:dart_ui/src/graphics/video/video_color_conversion.dart';
import 'package:dart_ui/src/graphics/video/video_frame.dart';

const width = 8;
const height = 4;
int luma(int x, int y) => (x * 37 + y * 53 + 16) & 255;
int chromaU(int x, int y) => (x * 59 + y * 71 + 40) & 255;
int chromaV(int x, int y) => (x * 83 + y * 29 + 210) & 255;

VideoFrame makeFrame(VideoPixelFormat pixel, VideoColorSpace space,
    VideoColorRange range, int padding,
    {int frameWidth = width, int frameHeight = height}) {
  final width = frameWidth;
  final height = frameHeight;
  final format = VideoFrameFormat(
      pixelFormat: pixel,
      width: width,
      height: height,
      colorSpace: space,
      range: range);
  final planes = <VideoPlane>[];
  for (var i = 0; i < format.planeCount; i++) {
    final stride = format.planeMinBytesPerRow(i) + padding;
    final offset = padding == 0 ? 0 : 3;
    final bytes = Uint8List(offset + stride * format.planeHeight(i));
    bytes.fillRange(0, bytes.length, 173);
    planes.add(VideoPlane(bytes: bytes, bytesPerRow: stride, offset: offset));
  }
  final frame = VideoFrame(
      format: format, planes: planes, streamId: 1, sequence: padding);
  for (var y = 0; y < height; y++) {
    for (var x = 0; x < width; x++) {
      final p = planes[0];
      final row = p.offset + y * p.bytesPerRow;
      final u = chromaU(x ~/ 2, y ~/ 2);
      final v = chromaV(x ~/ 2, y ~/ 2);
      if (pixel == VideoPixelFormat.yuy2) {
        final index = row + (x ~/ 2) * 4;
        p.bytes[index + (x % 2) * 2] = luma(x, y);
        p.bytes[index + 1] = u;
        p.bytes[index + 3] = v;
      } else if (pixel.isYuv) {
        p.bytes[row + x] = luma(x, y);
        if (x % 2 == 0 && y % 2 == 0) {
          final c = planes[1];
          final index = c.offset + (y ~/ 2) * c.bytesPerRow;
          if (pixel == VideoPixelFormat.nv12) {
            c.bytes[index + x] = u;
            c.bytes[index + x + 1] = v;
          } else {
            c.bytes[index + x ~/ 2] = u;
            final d = planes[2];
            d.bytes[d.offset + (y ~/ 2) * d.bytesPerRow + x ~/ 2] = v;
          }
        }
      } else {
        final index = row + x * 4;
        final r = (x * 31 + y * 7) & 255;
        final b = (x * 11 + y * 61) & 255;
        p.bytes[index] = pixel == VideoPixelFormat.bgra8888 ? b : r;
        p.bytes[index + 1] = (x * 17 + y * 41) & 255;
        p.bytes[index + 2] = pixel == VideoPixelFormat.bgra8888 ? r : b;
        p.bytes[index + 3] = 255;
      }
    }
  }
  return frame;
}

void requireEqual(Uint8List a, Uint8List b, String label) {
  if (a.length != b.length) throw StateError('$label length');
  for (var i = 0; i < a.length; i++) {
    if (a[i] != b[i]) throw StateError('$label byte $i: ${a[i]} != ${b[i]}');
  }
}

// Checksums every output byte outside the conversion timer. Input and output
// buffers are allocated before warmup and retained for every measured frame.
// The framework still allocates its own per-call axis maps/internal objects;
// this portable probe cannot observe their allocation count.
void benchmarkVideo(List<String> args) {
  if ((args.length != 2 && args.length != 4 && args.length != 5) ||
      args[0] != '--benchmark') {
    throw ArgumentError(
        'usage: video_probe.dart [--benchmark iterations [width height [pixelFormat]]]');
  }
  final iterations = int.parse(args[1]);
  final frameWidth = args.length >= 4 ? int.parse(args[2]) : 1920;
  final frameHeight = args.length >= 4 ? int.parse(args[3]) : 1080;
  if (iterations < 1 ||
      iterations > 1000 ||
      frameWidth < 2 ||
      frameWidth > 3840 ||
      frameHeight < 2 ||
      frameHeight > 2160 ||
      frameWidth % 2 != 0 ||
      frameHeight % 2 != 0) {
    throw ArgumentError(
        'iterations must be 1..1000; dimensions must be even, width 2..3840 and height 2..2160');
  }
  var pixel = VideoPixelFormat.nv12;
  if (args.length == 5) {
    var found = false;
    for (final candidate in VideoPixelFormat.values) {
      if (candidate.name == args[4]) {
        pixel = candidate;
        found = true;
      }
    }
    if (!found) throw ArgumentError('unknown pixel format: ${args[4]}');
  }
  final frame = makeFrame(
      pixel, VideoColorSpace.bt709, VideoColorRange.limited, 32,
      frameWidth: frameWidth, frameHeight: frameHeight);
  final destination = Uint8List(frameWidth * frameHeight * 4);
  const warmup = 3;
  for (var i = 0; i < warmup; i++) {
    convertVideoFrameToRgba(frame, into: destination);
  }
  var checksum = 0;
  final conversion = Stopwatch();
  final total = Stopwatch()..start();
  for (var i = 0; i < iterations; i++) {
    conversion.start();
    final result = convertVideoFrameToRgba(frame, into: destination);
    conversion.stop();
    if (!identical(result, destination)) throw StateError('destination reuse');
    for (var index = 0; index < destination.length; index++) {
      checksum = (checksum * 31 + destination[index]) & 0x7fffffff;
    }
  }
  total.stop();
  var inputBytes = 0;
  for (final plane in frame.planes) {
    inputBytes += plane.bytes.length;
  }
  print(
      'benchmark iterations=$iterations width=$frameWidth height=$frameHeight pixelFormat=${pixel.name} colorSpace=bt709 range=limited warmup=$warmup checksum=$checksum microseconds=${conversion.elapsedMicroseconds} totalMicroseconds=${total.elapsedMicroseconds} inputBytes=$inputBytes outputBytes=${destination.length} allocations=unavailable');
  frame.release();
}

void main(List<String> args) {
  if (args.isNotEmpty) {
    benchmarkVideo(args);
    return;
  }
  for (final space in VideoColorSpace.values) {
    for (final range in VideoColorRange.values) {
      Uint8List? reference;
      for (final pixel in <VideoPixelFormat>[
        VideoPixelFormat.nv12,
        VideoPixelFormat.i420,
        VideoPixelFormat.yuy2,
      ]) {
        final tight = makeFrame(pixel, space, range, 0);
        final padded = makeFrame(pixel, space, range, 7);
        final rgba = convertVideoFrameToRgba(tight);
        requireEqual(rgba, convertVideoFrameToRgba(padded), 'stride');
        if (reference != null) requireEqual(rgba, reference, 'layout');
        reference = rgba;
        final matrix = YuvToRgbMatrix.of(space, range);
        for (var y = 0; y < height; y++) {
          for (var x = 0; x < width; x++) {
            final rgb = matrix.rgbFromCodes(
                luma(x, y), chromaU(x ~/ 2, y ~/ 2), chromaV(x ~/ 2, y ~/ 2));
            final index = (y * width + x) * 4;
            if ((rgba[index] - rgb.$1).abs() > 1 ||
                (rgba[index + 1] - rgb.$2).abs() > 1 ||
                (rgba[index + 2] - rgb.$3).abs() > 1 ||
                rgba[index + 3] != 255) {
              throw StateError('matrix tolerance at $x,$y');
            }
          }
        }
        final bgra =
            convertVideoFrameToRgba(padded, order: ImageChannelOrder.bgra);
        final half = convertVideoFrameToRgba(padded, opacity: 128);
        for (var i = 0; i < rgba.length; i += 4) {
          if (bgra[i] != rgba[i + 2] ||
              bgra[i + 1] != rgba[i + 1] ||
              bgra[i + 2] != rgba[i] ||
              bgra[i + 3] != 255 ||
              half[i] != premultiplyChannel(rgba[i], 128) ||
              half[i + 1] != premultiplyChannel(rgba[i + 1], 128) ||
              half[i + 2] != premultiplyChannel(rgba[i + 2], 128) ||
              half[i + 3] != 128) {
            throw StateError('channel/opacity contract at $i');
          }
        }
        final cropped = Uint8List(24 * height);
        cropped.fillRange(0, cropped.length, 201);
        final returned = convertVideoFrameToRgba(padded,
            region: const VideoRegion(2, 0, 6, height),
            into: cropped,
            bytesPerRow: 24);
        if (!identical(returned, cropped))
          throw StateError('destination reuse');
        for (var y = 0; y < height; y++) {
          for (var x = 0; x < 4; x++) {
            for (var channel = 0; channel < 4; channel++) {
              if (cropped[y * 24 + x * 4 + channel] !=
                  rgba[(y * width + x + 2) * 4 + channel]) {
                throw StateError('crop contract at $x,$y');
              }
            }
          }
          for (var i = 16; i < 24; i++) {
            if (cropped[y * 24 + i] != 201)
              throw StateError('destination padding');
          }
        }
        print('${space.name}/${range.name}/${pixel.name}:${rgba.join(',')}');
        tight.release();
        padded.release();
      }
    }
  }
  final rgbaFrame = makeFrame(VideoPixelFormat.rgba8888, VideoColorSpace.bt709,
      VideoColorRange.full, 7);
  final bgraFrame = makeFrame(VideoPixelFormat.bgra8888, VideoColorSpace.bt709,
      VideoColorRange.full, 7);
  final packed = convertVideoFrameToRgba(rgbaFrame);
  requireEqual(
      packed, convertVideoFrameToRgba(bgraFrame), 'packed channel order');
  print('packed:${packed.join(',')}');
  rgbaFrame.release();
  bgraFrame.release();
  print('video contracts passed');
}
