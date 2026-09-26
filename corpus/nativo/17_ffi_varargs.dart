// dart:ffi: funções variádicas (VarArgs) pela ABI C do alvo — snprintf da
// libc com inteiros, double, float (promovido a double), inteiros estreitos
// (promovidos a int) e ponteiros na parte variádica, e VarArgs de um tipo só (`(T,)`).
import 'dart:ffi';


typedef MallocC = Pointer<Void> Function(IntPtr);
typedef MallocD = Pointer<Void> Function(int);
final _malloc = DynamicLibrary.process().lookupFunction<MallocC, MallocD>('malloc');

Pointer<Uint8> texto(String s) {
  final p = _malloc(s.length + 1).cast<Uint8>();
  for (var i = 0; i < s.length; i++) {
    p[i] = s.codeUnitAt(i);
  }
  p[s.length] = 0;
  return p;
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
  final buf = _malloc(256).cast<Uint8>();

  final f1 = libc.lookupFunction<Int32 Function(Pointer<Uint8>, Size, Pointer<Uint8>, VarArgs<(Int32, Double, Pointer<Uint8>)>),
      int Function(Pointer<Uint8>, int, Pointer<Uint8>, int, double, Pointer<Uint8>)>('snprintf');
  final n1 = f1(buf, 256, texto('%d|%.3f|%s'), -42, 3.14159, texto('ffi'));
  print([n1, textoC(buf)]);

  final f2 = libc.lookupFunction<Int32 Function(Pointer<Uint8>, Size, Pointer<Uint8>, VarArgs<(Int64, Float, Int8, Uint16, Double)>),
      int Function(Pointer<Uint8>, int, Pointer<Uint8>, int, double, int, int, double)>('snprintf');
  final n2 = f2(buf, 256, texto('%lld %.2f %d %u %g'), 1 << 40, 1.5, -7, 65535, 1e-3);
  print([n2, textoC(buf)]);

  final f3 = libc.lookupFunction<Int32 Function(Pointer<Uint8>, Size, Pointer<Uint8>, VarArgs<(Int64,)>),
      int Function(Pointer<Uint8>, int, Pointer<Uint8>, int)>('snprintf');
  f3(buf, 256, texto('[%lld]'), 123456789012);
  print(textoC(buf));

  // Muitos doubles: além dos registradores de vetor, vão para a pilha.
  final f4 = libc.lookupFunction<
      Int32 Function(Pointer<Uint8>, Size, Pointer<Uint8>, VarArgs<(Double, Double, Double, Double, Double, Double, Double, Double, Double, Int32)>),
      int Function(Pointer<Uint8>, int, Pointer<Uint8>, double, double, double, double, double, double, double, double, double, int)>('snprintf');
  f4(buf, 256, texto('%g %g %g %g %g %g %g %g %g %d'), 1, 2, 3, 4, 5, 6, 7, 8, 9, 10);
  print(textoC(buf));
}
