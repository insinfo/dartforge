/// Portable SIMD extensions recognized by DartForge's native backend.
///
/// ```dart
/// import 'dart:typed_data';
/// import 'package:dartforge/simd.dart';
///
/// void main() {
///   final a = Int32x4(1, -2, 3, -4);
///   final b = Int32x4(5, 6, 7, 8);
///   print((a * b).x); // 5
/// }
/// ```
library;

export 'src/simd/int32x4_3_14.dart';
