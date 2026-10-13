/// Dart 3.14's `Int32x4` API for code that still compiles with the 3.6.2
/// SDK (docs/SIMD-NATIVO.md §6).
///
/// The members have the exact signatures of `sdk/lib/typed_data/typed_data.dart`
/// on the SDK's `main` (3.14.0-dev; `Int32x4.operator *` is still under
/// review, CL 551260) and the bodies of the VM's `typed_data_patch.dart` — the
/// same results lane by lane, including wraparound on overflow and `s & 31`
/// in shifts. So the same program:
///
/// * on the 3.6.2 VM (the oracle), runs these bodies in Dart;
/// * on native DartForge, turns each call into an LLVM vector instruction
///   (`<4 x i32>`), recognized by the extension's
///   `@pragma('dartforge:simd-api', '3.14')`;
/// * on a 3.14 SDK, uses the class's own members, which take precedence over
///   extension members (the extension only fills in what is missing, like `*`).
///
/// The constructors `Int32x4.splat(v)` and `Int32x4.zero()` do not fit in a
/// 3.6 extension: use `Int32x4(v, v, v, v)` and `Int32x4(0, 0, 0, 0)`.
library;

import 'dart:typed_data';

@pragma('dartforge:simd-api', '3.14')
extension Int32x4Api314 on Int32x4 {
  /// Inverts every bit of each lane.
  Int32x4 operator ~() => Int32x4(~x, ~y, ~z, ~w);

  /// `this & ~other`, lane by lane.
  Int32x4 andNot(Int32x4 other) =>
      Int32x4(x & ~other.x, y & ~other.y, z & ~other.z, w & ~other.w);

  /// The negation of each lane, `(-n).toSigned(32)`.
  Int32x4 operator -() => Int32x4(-x, -y, -z, -w);

  /// The 32-bit `abs` of each lane: that of `-0x80000000` is itself.
  Int32x4 abs() => Int32x4(x.abs(), y.abs(), z.abs(), w.abs());

  /// The low 32 bits of each lane's product (CL 551260).
  Int32x4 operator *(Int32x4 other) =>
      Int32x4(x * other.x, y * other.y, z * other.z, w * other.w);

  /// Shifts each lane left by `shiftAmount % 32`.
  Int32x4 operator <<(int shiftAmount) {
    final int n = shiftAmount & 31;
    return Int32x4(x << n, y << n, z << n, w << n);
  }

  /// Arithmetic right shift of each lane by `shiftAmount % 32`.
  Int32x4 operator >>(int shiftAmount) {
    final int n = shiftAmount & 31;
    return Int32x4(x >> n, y >> n, z >> n, w >> n);
  }

  /// -1 where the lanes are equal, 0 elsewhere.
  Int32x4 equal(Int32x4 other) => Int32x4(
        x == other.x ? -1 : 0,
        y == other.y ? -1 : 0,
        z == other.z ? -1 : 0,
        w == other.w ? -1 : 0,
      );

  /// -1 where the lanes differ, 0 elsewhere.
  Int32x4 notEqual(Int32x4 other) => Int32x4(
        x != other.x ? -1 : 0,
        y != other.y ? -1 : 0,
        z != other.z ? -1 : 0,
        w != other.w ? -1 : 0,
      );

  /// -1 where `this < other` (signed), 0 elsewhere.
  Int32x4 lessThan(Int32x4 other) => Int32x4(
        x < other.x ? -1 : 0,
        y < other.y ? -1 : 0,
        z < other.z ? -1 : 0,
        w < other.w ? -1 : 0,
      );

  /// -1 where `this <= other` (signed), 0 elsewhere.
  Int32x4 lessThanOrEqual(Int32x4 other) => Int32x4(
        x <= other.x ? -1 : 0,
        y <= other.y ? -1 : 0,
        z <= other.z ? -1 : 0,
        w <= other.w ? -1 : 0,
      );

  /// -1 where `this > other` (signed), 0 elsewhere.
  Int32x4 greaterThan(Int32x4 other) => Int32x4(
        x > other.x ? -1 : 0,
        y > other.y ? -1 : 0,
        z > other.z ? -1 : 0,
        w > other.w ? -1 : 0,
      );

  /// -1 where `this >= other` (signed), 0 elsewhere.
  Int32x4 greaterThanOrEqual(Int32x4 other) => Int32x4(
        x >= other.x ? -1 : 0,
        y >= other.y ? -1 : 0,
        z >= other.z ? -1 : 0,
        w >= other.w ? -1 : 0,
      );

  /// The smaller of each pair of lanes (signed).
  Int32x4 min(Int32x4 other) => Int32x4(
        x < other.x ? x : other.x,
        y < other.y ? y : other.y,
        z < other.z ? z : other.z,
        w < other.w ? w : other.w,
      );

  /// The larger of each pair of lanes (signed).
  Int32x4 max(Int32x4 other) => Int32x4(
        x > other.x ? x : other.x,
        y > other.y ? y : other.y,
        z > other.z ? z : other.z,
        w > other.w ? w : other.w,
      );

  /// Some lane has a bit set.
  bool get anyTrue => (x | y | z | w) != 0;

  /// Every lane has a bit set.
  bool get allTrue => flagX && flagY && flagZ && flagW;
}
