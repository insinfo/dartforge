// dart:ffi: structs por valor numa chamada nativa, pela ABI C do alvo — o
// retorno de div/ldiv/lldiv da libc (div_t, ldiv_t, lldiv_t) e o argumento
// de inet_ntoa (struct in_addr), com os campos lidos da struct devolvida
// (sobre memória do heap Dart, como na VM).
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
}
