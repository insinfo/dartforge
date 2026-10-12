// D9 (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3): a exceção Dart que sai
// de um callback da FFI. O `qsort` da libc chama o comparador por um quadro
// C; o comparador aloca e lança quando os dois valores são iguais, e a
// entrada do callback devolve o retorno excepcional (0, "iguais") sem que o
// desenrolamento atravesse o quadro C. A ordem final não depende de quais
// pares o `qsort` compara. O callback chamado pelo próprio Dart, pelo
// ponteiro nativo, também lança. Depois, alocações sob coleta.
//
// Saída: [1, 1, 2, 3, 3, 3, 5, 8, 8, 9] 0 14 2000
import 'dart:ffi';

typedef CmpC = Int32 Function(Pointer<Int32>, Pointer<Int32>);
typedef QsortC = Void Function(Pointer<Int32>, IntPtr, IntPtr, Pointer<NativeFunction<CmpC>>);
typedef QsortD = void Function(Pointer<Int32>, int, int, Pointer<NativeFunction<CmpC>>);
typedef MallocC = Pointer<Void> Function(IntPtr);
typedef MallocD = Pointer<Void> Function(int);
typedef FreeC = Void Function(Pointer<Void>);
typedef FreeD = void Function(Pointer<Void>);
typedef DobroC = Int32 Function(Int32);
typedef DobroD = int Function(int);

final _lib = DynamicLibrary.process();

int comparar(Pointer<Int32> a, Pointer<Int32> b) {
  final s = 'c${a.value}-${b.value}';
  if (a.value == b.value) throw StateError(s);
  return a.value - b.value;
}

int dobro(int x) {
  final partes = List<String>.generate(4, (i) => 'd$i$x');
  if (x < 0) throw ArgumentError(partes.join());
  return x * 2;
}

void main() {
  final qsort = _lib.lookupFunction<QsortC, QsortD>('qsort');
  final malloc = _lib.lookupFunction<MallocC, MallocD>('malloc');
  final free = _lib.lookupFunction<FreeC, FreeD>('free');
  final xs = [3, 8, 1, 9, 3, 5, 1, 8, 3, 2];
  final p = malloc(4 * xs.length).cast<Int32>();
  for (var i = 0; i < xs.length; i++) {
    p[i] = xs[i];
  }
  qsort(p, xs.length, 4, Pointer.fromFunction<CmpC>(comparar, 0));
  final d = Pointer.fromFunction<DobroC>(dobro, 0).asFunction<DobroD>();
  final r1 = d(-5);
  final r2 = d(7);
  var n = 0;
  for (var i = 0; i < 2000; i++) {
    final l = [for (var j = 0; j < 8; j++) 'x$j$i'];
    n += l.length ~/ 8;
  }
  print('${[for (var i = 0; i < xs.length; i++) p[i]]} $r1 $r2 $n');
  free(p.cast());
}
