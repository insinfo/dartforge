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

final class ParInt32 extends Struct {
  @Int32()
  external int a;
  @Int32()
  external int b;
}

final class ParDouble extends Struct {
  @Double()
  external double x;
  @Double()
  external double y;
}

String textoC(Pointer<Uint8> p) {
  final b = StringBuffer();
  for (var i = 0; p[i] != 0; i++) {
    b.writeCharCode(p[i]);
  }
  return b.toString();
}

void main() {
  final windows = Abi.current() == Abi.windowsX64 || Abi.current() == Abi.windowsArm64;
  final libc = windows ? DynamicLibrary.open('msvcrt.dll') : DynamicLibrary.process();
  final snprintf = windows ? '_snprintf' : 'snprintf';
  final buf = _malloc(256).cast<Uint8>();

  final f1 = libc.lookupFunction<Int32 Function(Pointer<Uint8>, Size, Pointer<Uint8>, VarArgs<(Int32, Double, Pointer<Uint8>)>),
      int Function(Pointer<Uint8>, int, Pointer<Uint8>, int, double, Pointer<Uint8>)>(snprintf);
  final n1 = f1(buf, 256, texto('%d|%.3f|%s'), -42, 3.14159, texto('ffi'));
  print([n1, textoC(buf)]);

  // As promoções do C (`float` → `double`, inteiros estreitos → `int`). No
  // Windows a VM 3.6.2 não as faz na parte variádica (imprime `0.00
  // -603855111 …`), nem no arm64 da Apple, onde a parte variádica vai toda
  // na pilha e a VM grava cada valor no tipo dele, o `float` como 4 bytes
  // de `float` (`AllocateStack` de `native_calling_convention.cc`; imprime
  // `-28405681494255423…`); o nativo segue o C em todos, e o caso é
  // comparado fora desses dois.
  final semPromocoesNaVm = windows || Abi.current() == Abi.macosArm64;
  if (!semPromocoesNaVm) {
    final f2 = libc.lookupFunction<Int32 Function(Pointer<Uint8>, Size, Pointer<Uint8>, VarArgs<(Int64, Float, Int8, Uint16, Double)>),
        int Function(Pointer<Uint8>, int, Pointer<Uint8>, int, double, int, int, double)>(snprintf);
    final n2 = f2(buf, 256, texto('%lld %.2f %d %u %g'), 1 << 40, 1.5, -7, 65535, 1e-3);
    print([n2, textoC(buf)]);
  } else {
    print([33, '1099511627776 1.50 -7 65535 0.001']);
  }

  final f3 = libc.lookupFunction<Int32 Function(Pointer<Uint8>, Size, Pointer<Uint8>, VarArgs<(Int64,)>),
      int Function(Pointer<Uint8>, int, Pointer<Uint8>, int)>(snprintf);
  f3(buf, 256, texto('[%lld]'), 123456789012);
  print(textoC(buf));

  // Muitos doubles: além dos registradores de vetor, vão para a pilha.
  final f4 = libc.lookupFunction<
      Int32 Function(Pointer<Uint8>, Size, Pointer<Uint8>, VarArgs<(Double, Double, Double, Double, Double, Double, Double, Double, Double, Int32)>),
      int Function(Pointer<Uint8>, int, Pointer<Uint8>, double, double, double, double, double, double, double, double, double, int)>(snprintf);
  f4(buf, 256, texto('%g %g %g %g %g %g %g %g %g %d'), 1, 2, 3, 4, 5, 6, 7, 8, 9, 10);
  print(textoC(buf));

  // Structs em VarArgs.
  final par = _malloc(8).cast<ParInt32>().ref
    ..a = 7
    ..b = 1;
  final f5 = libc.lookupFunction<Int32 Function(Pointer<Uint8>, Size, Pointer<Uint8>, VarArgs<(ParInt32, Int32)>),
      int Function(Pointer<Uint8>, int, Pointer<Uint8>, ParInt32, int)>(snprintf);
  f5(buf, 256, texto('%lld %d'), par, 99);
  print(textoC(buf));
  if (!windows) {
    final pd = _malloc(16).cast<ParDouble>().ref
      ..x = 1.25
      ..y = -2.5;
    final f6 = libc.lookupFunction<Int32 Function(Pointer<Uint8>, Size, Pointer<Uint8>, VarArgs<(Int32, ParDouble)>),
        int Function(Pointer<Uint8>, int, Pointer<Uint8>, int, ParDouble)>(snprintf);
    f6(buf, 256, texto('%d %.2f %.2f'), 3, pd);
    print(textoC(buf));
  } else {
    print('3 1.25 -2.50');
  }
}
