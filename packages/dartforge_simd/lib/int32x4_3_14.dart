/// A API de `Int32x4` do Dart 3.14 para código que ainda compila com o SDK
/// 3.6.2 (docs/SIMD-NATIVO.md §6).
///
/// Os membros têm as assinaturas exatas de `sdk/lib/typed_data/typed_data.dart`
/// do `main` do SDK (3.14.0-dev; `Int32x4.operator *` ainda está em revisão,
/// CL 551260) e os corpos do `typed_data_patch.dart` da VM — os mesmos
/// resultados pista a pista, inclusive a volta no estouro e `s & 31` nos
/// deslocamentos. Assim o mesmo programa:
///
/// * na VM 3.6.2 (o oráculo), roda estes corpos em Dart;
/// * no DartForge nativo, cada chamada vira uma instrução vetorial do LLVM
///   (`<4 x i32>`), reconhecida pelo `@pragma('dartforge:simd-api', '3.14')`
///   da extensão;
/// * num SDK 3.14, usa os membros da própria classe, que têm precedência
///   sobre os de extensão (a extensão só completa o que faltar, como o `*`).
///
/// Os construtores `Int32x4.splat(v)` e `Int32x4.zero()` não cabem numa
/// extensão do 3.6: use `Int32x4(v, v, v, v)` e `Int32x4(0, 0, 0, 0)`.
library;

import 'dart:typed_data';

@pragma('dartforge:simd-api', '3.14')
extension Int32x4Api314 on Int32x4 {
  /// Inverte todos os bits de cada pista.
  Int32x4 operator ~() => Int32x4(~x, ~y, ~z, ~w);

  /// `this & ~other`, pista a pista.
  Int32x4 andNot(Int32x4 other) =>
      Int32x4(x & ~other.x, y & ~other.y, z & ~other.z, w & ~other.w);

  /// A negação de cada pista, `(-n).toSigned(32)`.
  Int32x4 operator -() => Int32x4(-x, -y, -z, -w);

  /// O `abs` de cada pista em 32 bits: o de `-0x80000000` é ele mesmo.
  Int32x4 abs() => Int32x4(x.abs(), y.abs(), z.abs(), w.abs());

  /// Os 32 bits baixos do produto de cada pista (CL 551260).
  Int32x4 operator *(Int32x4 other) =>
      Int32x4(x * other.x, y * other.y, z * other.z, w * other.w);

  /// Deslocamento à esquerda de cada pista por `shiftAmount % 32`.
  Int32x4 operator <<(int shiftAmount) {
    final int n = shiftAmount & 31;
    return Int32x4(x << n, y << n, z << n, w << n);
  }

  /// Deslocamento aritmético à direita de cada pista por `shiftAmount % 32`.
  Int32x4 operator >>(int shiftAmount) {
    final int n = shiftAmount & 31;
    return Int32x4(x >> n, y >> n, z >> n, w >> n);
  }

  /// -1 onde as pistas são iguais, 0 onde não.
  Int32x4 equal(Int32x4 other) => Int32x4(
        x == other.x ? -1 : 0,
        y == other.y ? -1 : 0,
        z == other.z ? -1 : 0,
        w == other.w ? -1 : 0,
      );

  /// -1 onde as pistas diferem, 0 onde não.
  Int32x4 notEqual(Int32x4 other) => Int32x4(
        x != other.x ? -1 : 0,
        y != other.y ? -1 : 0,
        z != other.z ? -1 : 0,
        w != other.w ? -1 : 0,
      );

  /// -1 onde `this < other` (com sinal), 0 onde não.
  Int32x4 lessThan(Int32x4 other) => Int32x4(
        x < other.x ? -1 : 0,
        y < other.y ? -1 : 0,
        z < other.z ? -1 : 0,
        w < other.w ? -1 : 0,
      );

  /// -1 onde `this <= other` (com sinal), 0 onde não.
  Int32x4 lessThanOrEqual(Int32x4 other) => Int32x4(
        x <= other.x ? -1 : 0,
        y <= other.y ? -1 : 0,
        z <= other.z ? -1 : 0,
        w <= other.w ? -1 : 0,
      );

  /// -1 onde `this > other` (com sinal), 0 onde não.
  Int32x4 greaterThan(Int32x4 other) => Int32x4(
        x > other.x ? -1 : 0,
        y > other.y ? -1 : 0,
        z > other.z ? -1 : 0,
        w > other.w ? -1 : 0,
      );

  /// -1 onde `this >= other` (com sinal), 0 onde não.
  Int32x4 greaterThanOrEqual(Int32x4 other) => Int32x4(
        x >= other.x ? -1 : 0,
        y >= other.y ? -1 : 0,
        z >= other.z ? -1 : 0,
        w >= other.w ? -1 : 0,
      );

  /// O menor de cada par de pistas (com sinal).
  Int32x4 min(Int32x4 other) => Int32x4(
        x < other.x ? x : other.x,
        y < other.y ? y : other.y,
        z < other.z ? z : other.z,
        w < other.w ? w : other.w,
      );

  /// O maior de cada par de pistas (com sinal).
  Int32x4 max(Int32x4 other) => Int32x4(
        x > other.x ? x : other.x,
        y > other.y ? y : other.y,
        z > other.z ? z : other.z,
        w > other.w ? w : other.w,
      );

  /// Alguma pista tem bit ligado.
  bool get anyTrue => (x | y | z | w) != 0;

  /// Todas as pistas têm bit ligado.
  bool get allTrue => flagX && flagY && flagZ && flagW;
}
