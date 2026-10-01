// Campos de struct anotados por um alias da classe nativa (`typedef DWORD =
// Uint32;` e `@DWORD()`, o estilo das estruturas do Win32) e por um nome
// com prefixo (`@ffi.Int16()`): o layout sai da classe que a anotação
// nomeia no escopo, não do texto. Antes a struct ficava sem layout e o
// `.ref` lançava "struct or union not registered" (o /metrics do
// new_sali/backend; docs/NATIVO-PROJETOS-REAIS.md, C13).

import 'dart:ffi';
import 'dart:ffi' as ffi;

typedef DWORD = Uint32;
typedef LONG = Int32;
typedef PVOID = Pointer<Void>;

final class FILETIME extends Struct {
  @DWORD()
  external int dwLowDateTime;
  @DWORD()
  external int dwHighDateTime;
}

final class Misto extends Struct {
  @ffi.Int16()
  external int curto;
  @LONG()
  external int longo;
  external PVOID ponteiro;
  external FILETIME tempo;
}

typedef CallocC = Pointer<Void> Function(IntPtr, IntPtr);
typedef CallocD = Pointer<Void> Function(int, int);
typedef FreeC = Void Function(Pointer<Void>);
typedef FreeD = void Function(Pointer<Void>);

final _lib = DynamicLibrary.process();
final _calloc = _lib.lookupFunction<CallocC, CallocD>('calloc');
final _free = _lib.lookupFunction<FreeC, FreeD>('free');

void main() {
  print('${sizeOf<FILETIME>()} ${sizeOf<Misto>()}');
  final f = _calloc(1, sizeOf<FILETIME>()).cast<FILETIME>();
  f.ref.dwLowDateTime = 0xFFFFFFFF;
  f.ref.dwHighDateTime = 7;
  print('${f.ref.dwLowDateTime} ${f.ref.dwHighDateTime}');
  final m = _calloc(1, sizeOf<Misto>()).cast<Misto>();
  m.ref.curto = -2;
  m.ref.longo = -70000;
  m.ref.tempo.dwHighDateTime = 9;
  print('${m.ref.curto} ${m.ref.longo} ${m.ref.ponteiro.address} ${m.ref.tempo.dwHighDateTime}');
  _free(f.cast());
  _free(m.cast());
}
