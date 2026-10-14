/// Bounded hints for automatic collection in the current isolate.
/// Explicit collection, stress tests and the hard heap ceiling take precedence.
abstract final class GarbageCollector {
  static bool get isSupported => _gcSupportedV1();

  /// Isolate-local monotonic elapsed time, for use with [deferUntil].
  static Duration get monotonicTime =>
      Duration(microseconds: _gcElapsedMicrosV1());

  /// Observable completed collections; null on the official Dart VM.
  static int? get collectionCount => isSupported ? _gcCollectionsV1() : null;

  /// Requests a limited delay of automatic collection.
  /// Returns false when unsupported or existing collection debt prevents rearming.
  /// Repeated requests cannot extend the current deadline or memory budget.
  /// This does not defer ARC releases, finalizers or certify realtime execution.
  static bool deferFor(Duration duration,
      {int maxExtraBytes = 4 * 1024 * 1024}) {
    if (duration.inMicroseconds <= 0) {
      throw ArgumentError.value(duration, 'duration', 'Must be positive');
    }
    if (maxExtraBytes <= 0) {
      throw ArgumentError.value(
          maxExtraBytes, 'maxExtraBytes', 'Must be positive');
    }
    return _gcDeferV1(duration.inMicroseconds, maxExtraBytes) == 1;
  }

  /// Requests deferral until an isolate-local [monotonicTime] deadline.
  /// A deadline that has already passed cancels the current deferral.
  static bool deferUntil(Duration deadline,
      {int maxExtraBytes = 4 * 1024 * 1024}) {
    if (maxExtraBytes <= 0) {
      throw ArgumentError.value(
          maxExtraBytes, 'maxExtraBytes', 'Must be positive');
    }
    final remaining = deadline.inMicroseconds - _gcElapsedMicrosV1();
    if (remaining <= 0) {
      cancelDeferral();
      return false;
    }
    return _gcDeferV1(remaining, maxExtraBytes) == 1;
  }

  /// Ends the deferral; existing collection debt still needs to be serviced.
  static void cancelDeferral() => _gcCancelV1();

  /// Requests one collection before a genuinely idle event-loop wait.
  /// It respects an active deferral and does not keep the isolate alive itself.
  static void collectWhenIdle() => _gcRequestIdleV1();
}

@pragma('dartforge:runtime-policy-v1')
bool _gcSupportedV1() => false;

@pragma('dartforge:runtime-policy-v1')
int _gcElapsedMicrosV1() => 0;

@pragma('dartforge:runtime-policy-v1')
int _gcDeferV1(int durationMicros, int maxExtraBytes) => 0;

@pragma('dartforge:runtime-policy-v1')
void _gcCancelV1() {}

@pragma('dartforge:runtime-policy-v1')
void _gcRequestIdleV1() {}

@pragma('dartforge:runtime-policy-v1')
int _gcCollectionsV1() => 0;
