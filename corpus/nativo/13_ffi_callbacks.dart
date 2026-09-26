// dart:ffi: callbacks nativos — Pointer.fromFunction (um trampolim por
// sítio, retorno excepcional), NativeCallable.isolateLocal (closures,
// ponteiros, close() dentro do próprio callback) e NativeCallable.listener
// chamado de outra thread; o qsort da libc chamando Dart.
import 'dart:async';
import 'dart:ffi';
import 'dart:io';

typedef CmpC = Int32 Function(Pointer<Int32>, Pointer<Int32>);
typedef QsortC = Void Function(Pointer<Int32>, IntPtr, IntPtr, Pointer<NativeFunction<CmpC>>);
typedef QsortD = void Function(Pointer<Int32>, int, int, Pointer<NativeFunction<CmpC>>);
typedef MallocC = Pointer<Void> Function(IntPtr);
typedef MallocD = Pointer<Void> Function(int);
typedef FreeC = Void Function(Pointer<Void>);
typedef FreeD = void Function(Pointer<Void>);
typedef DblC = Double Function(Double, Float, Int8, Uint16);
typedef DblD = double Function(double, double, int, int);
typedef BoolC = Bool Function(Bool, Int64);
typedef BoolD = bool Function(bool, int);
typedef PtrC = Pointer<Int32> Function(Pointer<Int32>, IntPtr);
typedef PtrD = Pointer<Int32> Function(Pointer<Int32>, int);
typedef U8C = Uint8 Function(Int32);
typedef U8D = int Function(int);
typedef VoidC = Void Function(Int32);
typedef VoidD = void Function(int);
typedef ThreadC = Void Function(Pointer<Void>);

final _lib = DynamicLibrary.process();
final _malloc = _lib.lookupFunction<MallocC, MallocD>('malloc');
final _free = _lib.lookupFunction<FreeC, FreeD>('free');

int crescente(Pointer<Int32> a, Pointer<Int32> b) => a.value - b.value;
double soma(double a, double b, int c, int d) => a + b + c + d;
bool nega(bool b, int x) => !b && x > 10;
int dobro(int x) {
  if (x < 0) throw StateError('negativo');
  return x * 2;
}

List<int> ler(Pointer<Int32> p, int n) => [for (var i = 0; i < n; i++) p[i]];

/// Roda `f(arg)` numa thread nativa nova e espera ela terminar.
void emOutraThread(Pointer<NativeFunction<ThreadC>> f, int arg) {
  if (Platform.isWindows) {
    final k = DynamicLibrary.open('kernel32.dll');
    final criar = k.lookupFunction<
        IntPtr Function(Pointer<Void>, IntPtr, Pointer<NativeFunction<ThreadC>>, Pointer<Void>, Uint32, Pointer<Void>),
        int Function(Pointer<Void>, int, Pointer<NativeFunction<ThreadC>>, Pointer<Void>, int, Pointer<Void>)>('CreateThread');
    final esperar = k.lookupFunction<Uint32 Function(IntPtr, Uint32), int Function(int, int)>('WaitForSingleObject');
    final fechar = k.lookupFunction<Int32 Function(IntPtr), int Function(int)>('CloseHandle');
    final h = criar(nullptr, 0, f, Pointer.fromAddress(arg), 0, nullptr);
    esperar(h, 0xFFFFFFFF);
    fechar(h);
    return;
  }
  final criar = _lib.lookupFunction<
      Int32 Function(Pointer<IntPtr>, Pointer<Void>, Pointer<NativeFunction<ThreadC>>, Pointer<Void>),
      int Function(Pointer<IntPtr>, Pointer<Void>, Pointer<NativeFunction<ThreadC>>, Pointer<Void>)>('pthread_create');
  final juntar = _lib.lookupFunction<Int32 Function(IntPtr, Pointer<Void>), int Function(int, Pointer<Void>)>('pthread_join');
  final tid = _malloc(8).cast<IntPtr>();
  criar(tid, nullptr, f, Pointer.fromAddress(arg));
  juntar(tid.value, nullptr);
  _free(tid.cast());
}

Future<void> main() async {
  final qsort = _lib.lookupFunction<QsortC, QsortD>('qsort');
  final p = _malloc(40).cast<Int32>();
  final xs = [5, 3, 9, 1, 7, 2, 8, 0, 6, 4];
  for (var i = 0; i < 10; i++) {
    p[i] = xs[i];
  }
  qsort(p, 10, 4, Pointer.fromFunction<CmpC>(crescente, 0));
  print(ler(p, 10));
  var chamadas = 0;
  final dec = NativeCallable<CmpC>.isolateLocal((Pointer<Int32> a, Pointer<Int32> b) {
    chamadas++;
    return b.value - a.value;
  }, exceptionalReturn: 0);
  qsort(p, 10, 4, dec.nativeFunction);
  print([ler(p, 10), chamadas > 0]);
  dec.close();

  final fd = Pointer.fromFunction<DblC>(soma, 0.0);
  print(fd.asFunction<DblD>()(1.5, 0.25, -3, 65535));
  final sitios = [for (var i = 0; i < 3; i++) Pointer.fromFunction<DblC>(soma, 0.0).address];
  print([sitios.toSet().length, sitios.first == fd.address]);
  final fb = Pointer.fromFunction<BoolC>(nega, false);
  print([fb.asFunction<BoolD>()(false, 11), fb.asFunction<BoolD>()(true, 11)]);
  final fl = Pointer.fromFunction<U8C>(dobro, 7);
  print([fl.asFunction<U8D>()(3), fl.asFunction<U8D>()(-1), fl.asFunction<U8D>()(200)]);

  p[2] = 42;
  final np = NativeCallable<PtrC>.isolateLocal((Pointer<Int32> q, int i) => q + i);
  final r = np.nativeFunction.asFunction<PtrD>()(p, 2);
  print([r.value, r.address - p.address]);
  np.close();

  var total = 0;
  late NativeCallable<VoidC> nv;
  nv = NativeCallable<VoidC>.isolateLocal((int x) {
    total += x;
    if (x == 3) nv.close();
  });
  final chamar = nv.nativeFunction.asFunction<VoidD>();
  chamar(1);
  chamar(2);
  chamar(3);
  print([total, nv.keepIsolateAlive]);

  final feito = Completer<List<int>>();
  final recebidos = <int>[];
  final ouvinte = NativeCallable<ThreadC>.listener((Pointer<Void> arg) {
    recebidos.add(arg.address);
    if (recebidos.length == 2) feito.complete(recebidos);
  });
  emOutraThread(ouvinte.nativeFunction, 7);
  emOutraThread(ouvinte.nativeFunction, 9);
  print((await feito.future)..sort());
  ouvinte.close();
  _free(p.cast());
}
