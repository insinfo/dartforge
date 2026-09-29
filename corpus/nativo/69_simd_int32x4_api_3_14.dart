// A API de `Int32x4` do Dart 3.14 (docs/SIMD-NATIVO.md §6) pela extensão
// marcada de `pacotes/dartforge_simd/lib/int32x4_3_14.dart` (copiada abaixo,
// igual): na VM 3.6.2 roda o Dart dos membros; no nativo, cada chamada é uma
// instrução `<4 x i32>`. Cobre os extremos (−2^31, 2^31−1), a volta no
// estouro, `s & 31` nos deslocamentos (inclusive negativos e ≥ 32), as
// máscaras das comparações com sinal, `anyTrue`/`allTrue`, e os caminhos
// em caixa (local capturado, parâmetro, `dynamic` não se aplica a extensão).
import 'dart:typed_data';

String i4(Int32x4 v) => '[${v.x}, ${v.y}, ${v.z}, ${v.w}]';

Int32x4 porParametro(Int32x4 a, Int32x4 b) => (a * b).andNot(a >> 3);

void main() {
  final a = Int32x4(-0x80000000, 0x7fffffff, -1, 12345);
  final b = Int32x4(3, -7, 0x10000, -12345);
  print('~ ${i4(~a)}');
  print('andNot ${i4(a.andNot(b))}');
  print('neg ${i4(-a)} abs ${i4(a.abs())}');
  print('mul ${i4(a * b)} ${i4(Int32x4(0x10000, 0x10000, 46341, -46341) * Int32x4(0x10000, 3, 46341, 46341))}');
  for (final s in [0, 1, 8, 31, 32, 33, -1, -33, 1 << 40]) {
    print('shift $s ${i4(a << s)} ${i4(a >> s)}');
  }
  print('eq ${i4(a.equal(b))} ${i4(a.equal(a))} ne ${i4(a.notEqual(b))}');
  print('lt ${i4(a.lessThan(b))} le ${i4(a.lessThanOrEqual(a))}');
  print('gt ${i4(a.greaterThan(b))} ge ${i4(a.greaterThanOrEqual(b))}');
  print('min ${i4(a.min(b))} max ${i4(a.max(b))}');
  final zero = Int32x4(0, 0, 0, 0);
  print('any ${zero.anyTrue} ${Int32x4(0, 0, 0, 4).anyTrue} all ${a.allTrue} ${a.withZ(0).allTrue}');
  print('flags ${a.lessThan(b).signMask} ${a.lessThan(b).select(Float32x4.splat(1), Float32x4.splat(2))}');

  // No laço, sobre listas SIMD (o caso do rasterizador).
  final src = Int32x4List(16), dst = Int32x4List(16);
  for (var i = 0; i < 16; i++) {
    src[i] = Int32x4(i * 1000 - 8000, -i * 77, i * i * 131071, 7 - i);
  }
  final limite = Int32x4(255, 255, 255, 255);
  var vazias = 0, cheias = 0;
  for (var i = 0; i < 16; i++) {
    final v = src[i];
    final m = v.greaterThan(zero);
    dst[i] = ((v.abs() << 8) - v.abs() >> 8).min(limite) & m | (-v).andNot(m);
    if (v.equal(zero).anyTrue) vazias++;
    if (v.greaterThanOrEqual(Int32x4(-8000, -2000, 0, -9)).allTrue) cheias++;
  }
  print('laco ${i4(dst[0])} ${i4(dst[9])} ${i4(dst[15])} $vazias $cheias');

  // Caminhos em caixa: parâmetro, retorno e local capturado.
  print('parametro ${i4(porParametro(a, b))}');
  var capturado = Int32x4(1, 2, 3, 4);
  void mexer() => capturado = (capturado << 2) * capturado;
  mexer();
  print('capturado ${i4(capturado)} ${capturado.lessThan(Int32x4(5, 20, 40, 60)).allTrue}');
}

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
