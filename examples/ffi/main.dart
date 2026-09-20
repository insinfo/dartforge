import 'dart:ffi' as ffi;

@ffi.Native<ffi.Int32 Function(ffi.Int32, ffi.Int32)>(symbol: 'somar_valores')
external int somarValores(int a, int b);

void main() {
  print(somarValores(20, 22));
}
