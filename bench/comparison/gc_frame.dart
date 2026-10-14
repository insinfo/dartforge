// Allocation-only frame probe. This is not a UI rasterizer or an HTTP workload.
import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:dartforge/gc.dart';

final class FrameItem {
  final int id;
  final Map<String, int> properties;
  final Uint8List pixels;
  FrameItem(this.id, this.properties, this.pixels);
}

int frameWork(int frame, int itemCount) {
  final items = <FrameItem>[];
  for (var i = 0; i < itemCount; i++) {
    final properties = <String, int>{
      'x': (i * 17 + frame) & 1023,
      'y': (i * 29 + frame * 3) & 1023,
      'alpha': (i + frame) & 255,
      'layer': i & 15,
    };
    final pixels = Uint8List(4096);
    for (var offset = 0; offset < pixels.length; offset += 64) {
      pixels[offset] = (i * 7 + offset + frame) & 255;
    }
    items.add(FrameItem(i, properties, pixels));
  }
  var checksum = 0;
  for (final item in items) {
    checksum += item.id + item.properties['x']! + item.properties['y']!;
    checksum += item.properties['alpha']! + item.properties['layer']!;
    for (var offset = 0; offset < item.pixels.length; offset += 64) {
      checksum += item.pixels[offset];
    }
  }
  return checksum;
}

Future<void> main(List<String> arguments) async {
  var deferred = false;
  var frames = 20;
  var items = 768;
  for (final argument in arguments) {
    if (argument == '--defer') {
      deferred = true;
    } else if (argument.startsWith('--frames=')) {
      frames = int.parse(argument.substring(9));
    } else if (argument.startsWith('--items=')) {
      items = int.parse(argument.substring(8));
    } else {
      throw ArgumentError('Unknown argument: $argument');
    }
  }
  if (frames < 10 || frames > 30 || items < 64 || items > 2048) {
    throw ArgumentError('Expected 10..30 frames and 64..2048 items');
  }
  final supported = GarbageCollector.isSupported;
  if (deferred && !supported) {
    throw UnsupportedError('This runtime does not support GC deferral');
  }
  const warmup = 4;
  const bufferBytes = 4096;
  const allowance = 32 * 1024 * 1024;
  final samples = <Map<String, Object?>>[];
  final stopwatch = Stopwatch();
  final idleStopwatch = Stopwatch();
  var checksum = 0;
  var timedCollections = 0;
  var idleCollections = 0;
  var elapsedMicros = 0;
  var idleMicros = 0;
  for (var turn = -warmup; turn < frames; turn++) {
    final frame = turn < 0 ? turn + warmup : turn;
    var accepted = false;
    if (deferred) {
      accepted = GarbageCollector.deferFor(const Duration(seconds: 1),
          maxExtraBytes: allowance);
      if (!accepted) throw StateError('GC deferral was not accepted');
    }
    final before = GarbageCollector.collectionCount;
    stopwatch.reset();
    stopwatch.start();
    final value = frameWork(frame, items);
    stopwatch.stop();
    final frameMicros = stopwatch.elapsedMicroseconds;
    final after = GarbageCollector.collectionCount;

    // Identical idle handling in both modes. Create the waiting timer before
    // canceling the window, so collector debt cannot enter the timed kernel.
    idleStopwatch.reset();
    idleStopwatch.start();
    final idle = Future<void>.delayed(const Duration(milliseconds: 5));
    GarbageCollector.cancelDeferral();
    GarbageCollector.collectWhenIdle();
    await idle;
    idleStopwatch.stop();
    final afterIdle = GarbageCollector.collectionCount;
    if (supported && afterIdle! <= after!) {
      throw StateError('Requested idle collection was not serviced');
    }
    if (turn >= 0) {
      final frameCollections = before == null ? null : after! - before;
      final idleDelta = after == null ? null : afterIdle! - after;
      checksum = (checksum * 31 + value) & 0x7fffffff;
      elapsedMicros += frameMicros;
      idleMicros += idleStopwatch.elapsedMicroseconds;
      timedCollections += frameCollections ?? 0;
      idleCollections += idleDelta ?? 0;
      samples.add(<String, Object?>{
        'frame': turn,
        'checksum': value,
        'elapsed_micros': frameMicros,
        'collections': frameCollections,
        'before_collections': before,
        'after_collections': after,
        'after_idle_collections': afterIdle,
        'idle_micros': idleStopwatch.elapsedMicroseconds,
        'idle_collections': idleDelta,
        'deferral_accepted': accepted,
      });
    }
  }
  print(jsonEncode(<String, Object?>{
    'schema': 1,
    'workload': 'objects_maps_typed_buffers',
    'deferred': deferred,
    'gc_supported': supported,
    'frames': frames,
    'warmup_frames': warmup,
    'items_per_frame': items,
    'typed_buffer_bytes_per_item': bufferBytes,
    'sample_stride': 64,
    'deferral_micros': 1000000,
    'max_extra_bytes': allowance,
    'checksum': checksum,
    'timed_micros': elapsedMicros,
    'timed_collections': supported ? timedCollections : null,
    'idle_micros': idleMicros,
    'idle_collections': supported ? idleCollections : null,
    'samples': samples,
  }));
}
