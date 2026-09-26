// dart:ffi: structs por valor pela ABI C do alvo — o retorno de
// div/ldiv/lldiv da libc (div_t, ldiv_t, lldiv_t, também por @Native) e o
// argumento de inet_ntoa (struct in_addr), com os campos lidos da struct
// devolvida (sobre memória do heap Dart, como na VM); e callbacks
// (fromFunction, NativeCallable) que recebem e devolvem structs de vários
// formatos (em registradores, na pilha, HFA, @Packed), chamados pelo
// asFunction do próprio ponteiro.
import 'dart:ffi';
import 'dart:io';
import 'dart:typed_data';

final class DivT extends Struct {
  @Int32()
  external int quot;
  @Int32()
  external int rem;
}

final class LDivT extends Struct {
  @Long()
  external int quot;
  @Long()
  external int rem;
}

final class LLDivT extends Struct {
  @LongLong()
  external int quot;
  @LongLong()
  external int rem;
}

final class D2 extends Struct {
  @Double()
  external double a;
  @Double()
  external double b;
}

final class G extends Struct {
  @Int64()
  external int a;
  @Int64()
  external int b;
  @Int64()
  external int c;
}

final class F3 extends Struct {
  @Float()
  external double a;
  @Float()
  external double b;
  @Float()
  external double c;
}

@Packed(1)
final class PK extends Struct {
  @Int8()
  external int a;
  @Int32()
  external int b;
}

@Native<DivT Function(Int32, Int32)>(symbol: 'div')
external DivT divNativo(int a, int b);

D2 troca(D2 x, int k, G g) => Struct.create<D2>()
  ..a = x.b + k
  ..b = x.a + g.a + g.b + g.c;

G dobra(G g) => Struct.create<G>()
  ..a = g.a * 2
  ..b = g.b * 2
  ..c = g.c * 2;

PK mistura(F3 f, PK p) => Struct.create<PK>()
  ..a = p.a + 1
  ..b = p.b + (f.a + f.b + f.c).toInt();

final class InAddr extends Struct {
  @Uint32()
  external int sAddr;
}

String textoC(Pointer<Uint8> p) {
  final b = StringBuffer();
  for (var i = 0; p[i] != 0; i++) {
    b.writeCharCode(p[i]);
  }
  return b.toString();
}

void main() {
  final libc = DynamicLibrary.process();
  final div = libc.lookupFunction<DivT Function(Int32, Int32), DivT Function(int, int)>('div');
  final ldiv = libc.lookupFunction<LDivT Function(Long, Long), LDivT Function(int, int)>('ldiv');
  final lldiv = libc.lookupFunction<LLDivT Function(LongLong, LongLong), LLDivT Function(int, int)>('lldiv');
  final d = div(17, 5);
  print([d.quot, d.rem, sizeOf<DivT>()]);
  final n = div(-17, 5);
  print([n.quot, n.rem]);
  final l = ldiv(1000000, 7);
  print([l.quot, l.rem]);
  final ll = lldiv(-9000000000000, 7);
  print([ll.quot, ll.rem, sizeOf<LLDivT>()]);

  // inet_ntoa(struct in_addr): o endereço em ordem de rede (127.0.0.1).
  final rede = Platform.isWindows ? DynamicLibrary.open('ws2_32.dll') : libc;
  final ntoa = rede.lookupFunction<Pointer<Uint8> Function(InAddr), Pointer<Uint8> Function(InAddr)>('inet_ntoa');
  final a = Struct.create<InAddr>()..sAddr = Endian.host == Endian.little ? 0x0100007f : 0x7f000001;
  print(textoC(ntoa(a)));
  final b = Struct.create<InAddr>()..sAddr = Endian.host == Endian.little ? 0x04030201 : 0x01020304;
  print(textoC(ntoa(b)));

  final dn = divNativo(100, 7);
  print([dn.quot, dn.rem]);

  // Callbacks com struct por valor, chamados pelo asFunction do ponteiro.
  final d2 = Struct.create<D2>()
    ..a = 1.5
    ..b = 2.5;
  final g = Struct.create<G>()
    ..a = 1
    ..b = 2
    ..c = 3;
  final ft = Pointer.fromFunction<D2 Function(D2, Int32, G)>(troca).asFunction<D2 Function(D2, int, G)>();
  final rt = ft(d2, 7, g);
  print([rt.a, rt.b]);
  final nc = NativeCallable<G Function(G)>.isolateLocal(dobra);
  final rg = nc.nativeFunction.asFunction<G Function(G)>()(g);
  print([rg.a, rg.b, rg.c]);
  nc.close();
  final f3 = Struct.create<F3>()
    ..a = 0.5
    ..b = 1.5
    ..c = 2.5;
  final pk = Struct.create<PK>()
    ..a = 9
    ..b = 1000;
  final rm = Pointer.fromFunction<PK Function(F3, PK)>(mistura).asFunction<PK Function(F3, PK)>()(f3, pk);
  print([rm.a, rm.b]);
}
