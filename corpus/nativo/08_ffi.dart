// dart:ffi: DynamicLibrary.process, lookupFunction/asFunction com inteiros, inteiros da ABI (Long, IntPtr),
// double e ponteiros; memória nativa por Pointer (índice, value, cast, aritmética), sizeOf e Abi.current.
import 'dart:ffi';

typedef AbsC = Int32 Function(Int32);
typedef AbsD = int Function(int);
typedef StrlenC = IntPtr Function(Pointer<Uint8>);
typedef StrlenD = int Function(Pointer<Uint8>);
typedef MallocC = Pointer<Void> Function(IntPtr);
typedef MallocD = Pointer<Void> Function(int);
typedef FreeC = Void Function(Pointer<Void>);
typedef FreeD = void Function(Pointer<Void>);
typedef SqrtC = Double Function(Double);
typedef SqrtD = double Function(double);
typedef LabsC = Long Function(Long);
typedef LabsD = int Function(int);

void main() {
  final lib = DynamicLibrary.process();
  final abs = lib.lookupFunction<AbsC, AbsD>('abs');
  print(abs(-42));
  print(abs(7));
  final labs = lib.lookupFunction<LabsC, LabsD>('labs');
  print(labs(-1234567890123));
  final malloc = lib.lookupFunction<MallocC, MallocD>('malloc');
  final free = lib.lookupFunction<FreeC, FreeD>('free');
  final strlen = lib.lookupFunction<StrlenC, StrlenD>('strlen');
  final sqrt = lib.lookupFunction<SqrtC, SqrtD>('sqrt');
  print(sqrt(2.0));
  final p = malloc(16).cast<Uint8>();
  final s = 'hello';
  for (var i = 0; i < s.length; i++) {
    p[i] = s.codeUnitAt(i);
  }
  p[s.length] = 0;
  print(strlen(p));
  print(p[1]);
  final q = p.cast<Int32>();
  q[2] = -5;
  print(q[2]);
  print(p.cast<Uint32>()[2]);
  final d = p.cast<Double>();
  d[1] = 3.25;
  print(d[1]);
  print((p + 1).address - p.address);
  print(p.elementAt(4).address - p.address);
  free(p.cast());
  print(sizeOf<Int32>());
  print(sizeOf<IntPtr>());
  print(sizeOf<Double>());
  print(sizeOf<Pointer<Int8>>());
  print(nullptr.address);
  print(lib.providesSymbol('strlen'));
  print(lib.providesSymbol('nao_existe_xyz'));
  print(Abi.current());
}
