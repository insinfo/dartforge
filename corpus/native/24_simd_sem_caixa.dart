// SIMD sem caixa (`lower/simd.rs`, `llvm/simd.rs`): `Float32x4`, `Int32x4` e
// `Float64x2` em locais viram vetores LLVM. Cobre as operações, os getters, os
// construtores, `shuffle` com constantes, `select`, `signMask`, as conversões,
// NaN e ±0 em `min`/`max`/`clamp`, listas SIMD, e os casos que ficam em
// caixa (local capturado por closure, local em função assíncrona).
import 'dart:typed_data';

String f4(Float32x4 v) => '[${v.x}, ${v.y}, ${v.z}, ${v.w}]';
String f2(Float64x2 v) => '[${v.x}, ${v.y}]';
String i4(Int32x4 v) => '[${v.x}, ${v.y}, ${v.z}, ${v.w}]';
String sinais(Float32x4 v) =>
    [v.x, v.y, v.z, v.w].map((d) => d.isNaN ? 'nan' : (d == 0 && d.isNegative ? '-0' : '$d')).join(' ');
String sinais2(Float64x2 v) =>
    [v.x, v.y].map((d) => d.isNaN ? 'nan' : (d == 0 && d.isNegative ? '-0' : '$d')).join(' ');

Future<void> assincrona(Float32x4 a) async {
  var b = a * a;
  await Future<void>.delayed(Duration.zero);
  b = b + a;
  print('async ${f4(b)}');
}

void main() async {
  // Aritmética de Float32x4.
  var a = Float32x4(1.5, -2.25, 3.0, 0.1);
  var b = Float32x4(0.5, 4.0, -1.0, 3.3);
  print('f4 + ${f4(a + b)}');
  print('f4 - ${f4(a - b)}');
  print('f4 * ${f4(a * b)}');
  print('f4 / ${f4(a / b)}');
  print('f4 neg ${f4(-a)}');
  print('f4 abs ${f4(a.abs())}');
  print('f4 sqrt ${f4(b.abs().sqrt())}');
  print('f4 recip ${f4(b.reciprocal())}');
  print('f4 rsqrt ${f4(b.abs().reciprocalSqrt())}');
  print('f4 scale ${f4(a.scale(2.5))}');
  print('f4 splat ${f4(Float32x4.splat(7.25))} zero ${f4(Float32x4.zero())}');
  print('f4 with ${f4(a.withX(9).withY(8).withZ(7).withW(6))}');

  // min/max/clamp com NaN e ±0: o resultado da plataforma, igual à VM.
  final nan = Float32x4(double.nan, 0.0, -0.0, 1.0);
  final z = Float32x4(1.0, -0.0, 0.0, double.nan);
  print('f4 min ${sinais(nan.min(z))} | ${sinais(z.min(nan))}');
  print('f4 max ${sinais(nan.max(z))} | ${sinais(z.max(nan))}');
  print('f4 clamp ${sinais(Float32x4(-5, 0.5, 5, double.nan).clamp(Float32x4.splat(0), Float32x4.splat(1)))}');

  // Comparações e select.
  final m = a.greaterThan(b);
  print('cmp gt ${i4(m)} lt ${i4(a.lessThan(b))} ge ${i4(a.greaterThanOrEqual(a))}');
  print('cmp le ${i4(a.lessThanOrEqual(b))} eq ${i4(a.equal(a))} ne ${i4(a.notEqual(b))}');
  print('select ${f4(m.select(a, b))} signMask ${a.signMask} ${m.signMask}');

  // Shuffle com as constantes e com literais.
  print('shuffle ${f4(a.shuffle(Float32x4.wzyx))} ${f4(a.shuffle(Float32x4.xxyy))} ${f4(a.shuffle(0x1B))}');
  print('shuffleMix ${f4(a.shuffleMix(b, Float32x4.xyxy))} ${f4(a.shuffleMix(b, 0xE4))}');

  // Int32x4: bits, flags, aritmética com estouro de 32 bits.
  var i = Int32x4(1, -2, 0x7FFFFFFF, -0x80000000);
  var j = Int32x4(3, 5, 1, -1);
  print('i4 + ${i4(i + j)} - ${i4(i - j)}');
  print('i4 & ${i4(i & j)} | ${i4(i | j)} ^ ${i4(i ^ j)}');
  print('i4 flags ${i.flagX} ${i.flagY} ${i.flagZ} ${i.flagW} signMask ${i.signMask}');
  print('i4 with ${i4(i.withX(10).withY(20).withZ(30).withW(40))}');
  print('i4 withFlag ${i4(i.withFlagX(false).withFlagY(true).withFlagZ(false).withFlagW(true))}');
  print('i4 bool ${i4(Int32x4.bool(true, false, true, false))}');
  print('i4 shuffle ${i4(i.shuffle(Int32x4.wzyx))} ${i4(i.shuffleMix(j, Int32x4.xyzw))}');

  // Conversões.
  print('bits ${i4(Int32x4.fromFloat32x4Bits(a))} ${f4(Float32x4.fromInt32x4Bits(Int32x4(0x3F800000, 0, -1, 0x40490FDB)))}');
  final d = Float64x2(1.25, -3.5);
  print('conv ${f4(Float32x4.fromFloat64x2(d))} ${f2(Float64x2.fromFloat32x4(a))}');

  // Float64x2.
  final e = Float64x2(0.1, 2.0);
  print('f2 ${f2(d + e)} ${f2(d - e)} ${f2(d * e)} ${f2(d / e)} ${f2(-d)} ${f2(d.abs())}');
  print('f2 ${f2(e.sqrt())} ${f2(d.scale(3))} ${f2(Float64x2.splat(4))} ${f2(Float64x2.zero())}');
  print('f2 with ${f2(d.withX(5).withY(6))} signMask ${d.signMask}');
  final n2 = Float64x2(double.nan, -0.0);
  final z2 = Float64x2(1.0, 0.0);
  print('f2 min ${sinais2(n2.min(z2))} | ${sinais2(z2.min(n2))} max ${sinais2(n2.max(z2))} | ${sinais2(z2.max(n2))}');
  print('f2 clamp ${sinais2(Float64x2(-1, 9).clamp(Float64x2(0, 0), Float64x2(1, 1)))}');

  // Laço com acumulador sem caixa e listas SIMD.
  final lista = Float32x4List(64);
  for (var k = 0; k < lista.length; k++) {
    lista[k] = Float32x4(k.toDouble(), k * 0.5, -k.toDouble(), 1);
  }
  var soma = Float32x4.zero();
  for (var k = 0; k < lista.length; k++) {
    soma = soma + lista[k] * Float32x4.splat(0.25);
  }
  print('soma ${f4(soma)}');
  final li = Int32x4List(8);
  for (var k = 0; k < li.length; k++) {
    li[k] = Int32x4(k, k * k, -k, 0x10000 * k);
  }
  var acc = Int32x4(0, 0, 0, 0);
  for (var k = 0; k < li.length; k++) {
    acc = acc + li[k];
  }
  print('acc ${i4(acc)} ${i4(li[3])}');
  final ld = Float64x2List(4);
  ld[1] = Float64x2(1.5, 2.5);
  ld[2] = ld[1] + ld[1];
  print('ld ${f2(ld[0])} ${f2(ld[2])} ${ld.length}');
  try {
    print(lista[64]);
  } on RangeError catch (e) {
    print('fora: ${e.runtimeType}');
  }

  // Local capturado (em caixa) e interpolação direta.
  var capturado = Float32x4.splat(1);
  void soma1() => capturado = capturado + Float32x4.splat(1);
  soma1();
  soma1();
  print('capturado ${f4(capturado)} $a');
  late Float32x4 tarde;
  tarde = a + a;
  print('late ${f4(tarde)}');
  Object o = a;
  print('objeto ${o is Float32x4} ${(o as Float32x4).x}');
  await assincrona(a);
}
