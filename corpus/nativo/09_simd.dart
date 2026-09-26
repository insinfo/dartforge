// SIMD do dart:typed_data (Float32x4, Int32x4, Float64x2): aritmética por
// pista em float, comparações, máscaras, shuffle, as listas e as visões, e o
// texto da VM (%f e %08x). Só nativo: na web o toString é outro.
import 'dart:typed_data';

void main() {
  final a = Float32x4(1.5, -2.25, 3.1, 0.1);
  final b = Float32x4.splat(2.0);
  print(a);
  print(a + b);
  print(a * b);
  print(a / b);
  print(-a);
  print(a.abs());
  print(a.scale(0.5));
  print(a.clamp(Float32x4.splat(-1), Float32x4.splat(2)));
  print(a.min(b));
  print(a.max(b));
  print(Float32x4(4, 9, 16, 25).sqrt());
  print(Float32x4(2, 4, 8, 16).reciprocal());
  print(a.signMask);
  print(a.x + a.y + a.z + a.w);
  print(a.shuffle(Float32x4.wzyx));
  print(a.shuffleMix(b, Float32x4.xyxy));
  print(a.withX(9.0).withW(-1.0));
  final m = a.greaterThan(b);
  print(m);
  print('${m.flagX} ${m.flagY} ${m.flagZ} ${m.signMask}');
  print(m.select(a, b));
  final i = Int32x4(1, -2, 0x7fffffff, 5);
  print(i + Int32x4(1, 1, 1, 1));
  print(i & Int32x4(3, 3, 3, 3));
  print(i.shuffle(Int32x4.yxwz));
  print(Int32x4.bool(true, false, true, false));
  print(Float32x4.fromInt32x4Bits(Int32x4(0x3f800000, 0, 0, 0)));
  print(Int32x4.fromFloat32x4Bits(Float32x4(1, 0, 0, 0)).x);
  final d = Float64x2(1.25, -3.5);
  print(d + Float64x2.splat(1));
  print(d.abs());
  print(d.signMask);
  print(Float64x2.fromFloat32x4(a));
  print(Float32x4.fromFloat64x2(d));
  final lista = Float32x4List(3);
  lista[1] = a;
  print(lista[1]);
  print(lista[0]);
  print(Float32List.view(lista.buffer)[5]);
  final il = Int32x4List.fromList([i, i]);
  print(il[1].w);
  final dl = Float64x2List(2)..[0] = d;
  print(dl[0].y);
  try {
    a.shuffle(300);
  } on RangeError catch (e) {
    print(e);
  }
  print(a is Float32x4);
  print(a.runtimeType);
}
