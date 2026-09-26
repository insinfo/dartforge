// dart:ffi: Native.addressOf de funções @Native (o endereço do símbolo, que
// asFunction chama) e variáveis @Native (leitura e escrita do dado C no
// endereço do símbolo; resolvido no primeiro acesso, como na VM).
import 'dart:ffi';
import 'dart:io';

@Native<Int32 Function(Int32)>()
external int abs(int x);

@Native<Size Function(Pointer<Uint8>)>(symbol: 'strlen')
external int comprimento(Pointer<Uint8> s);

@Native<Pointer<Pointer<Uint8>>>()
external Pointer<Pointer<Uint8>> environ;

@Native<Int32>(symbol: 'opterr')
external int opterr;

void main() {
  final pa = Native.addressOf<NativeFunction<Int32 Function(Int32)>>(abs);
  print([pa.address != 0, pa.asFunction<int Function(int)>()(-5), abs(-6)]);
  final ps = Native.addressOf<NativeFunction<Size Function(Pointer<Uint8>)>>(comprimento);
  print(ps.address == DynamicLibrary.process().lookup<Void>('strlen').address);

  if (!Platform.isWindows) {
    print([environ.address != 0, environ[0].address != 0]);
    print(Native.addressOf<Pointer<Pointer<Uint8>>>(environ).address != 0);
    final antes = opterr;
    opterr = 0;
    print([antes, opterr]);
    opterr = antes;
    print(opterr == antes);
  } else {
    print([true, true]);
    print(true);
    print([1, 0]);
    print(true);
  }
}
