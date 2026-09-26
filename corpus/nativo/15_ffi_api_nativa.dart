// dart:ffi NativeApi: Dart_PostCObject e Dart_PostInteger (pelos ponteiros de
// NativeApi, com um Dart_CObject montado em memória nativa) chegando a uma
// porta, SendPort.nativePort, Dart_CloseNativePort de uma porta inexistente e
// os dados de initializeApiDLData (versão e tabela de funções).
import 'dart:ffi';
import 'dart:isolate';
import 'dart:typed_data';

typedef MallocC = Pointer<Void> Function(IntPtr);
typedef MallocD = Pointer<Void> Function(int);

final _malloc = DynamicLibrary.process().lookupFunction<MallocC, MallocD>('malloc');

/// Um Dart_CObject (type int32 em 0, a união em 8; 48 bytes).
Pointer<Uint8> cobject(int tipo) {
  final p = _malloc(48).cast<Uint8>();
  for (var i = 0; i < 48; i++) {
    p[i] = 0;
  }
  p.cast<Int32>().value = tipo;
  return p;
}

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
  print([NativeApi.majorVersion, NativeApi.minorVersion]);

  // A tabela de initializeApiDLData: {int major, int minor, entries*}.
  final dados = NativeApi.initializeApiDLData.cast<Int32>();
  final entradas = (dados + 2).cast<Pointer<Pointer<Uint8>>>().value;
  final nomes = <String>{};
  for (var i = 0; entradas[2 * i] != nullptr; i++) {
    nomes.add(textoC(entradas[2 * i]));
  }
  print([dados[0], dados[1], nomes.contains('Dart_PostCObject'), nomes.contains('Dart_NewNativePort')]);

  final post = NativeApi.postCObject.cast<NativeFunction<Bool Function(Int64, Pointer<Uint8>)>>()
      .asFunction<bool Function(int, Pointer<Uint8>)>();
  final fechar = NativeApi.closeNativePort.asFunction<int Function(int)>();

  final recebidas = <Object?>[];
  late RawReceivePort rp;
  rp = RawReceivePort((m) {
    recebidas.add(m);
    if (recebidas.length == 2) {
      final l = recebidas[0] as List;
      print([l[0], l[1], l[2], l[3], l[4], l[4] is Uint16List, l[5]]);
      print(recebidas[1]);
      rp.close();
    }
  });
  final porta = rp.sendPort.nativePort;

  // [ "olá", 42, true, 1.5, Uint16List[1, 65535], null ]
  final itens = [
    cobject(5)..cast<Pointer<Uint8>>().elementAt(1).value = texto('ola'),
    cobject(3)..cast<Int64>().elementAt(1).value = 42,
    cobject(1)..elementAt(8).value = 1,
    cobject(4)..cast<Double>().elementAt(1).value = 1.5,
    cobject(7),
    cobject(0),
  ];
  final u16 = _malloc(4).cast<Uint16>();
  u16[0] = 1;
  u16[1] = 65535;
  itens[4].cast<Int32>().elementAt(2).value = 5; // Dart_TypedData_kUint16
  itens[4].cast<Int64>().elementAt(2).value = 2;
  itens[4].cast<Pointer<Uint16>>().elementAt(3).value = u16;
  final vetor = _malloc(8 * itens.length).cast<Pointer<Uint8>>();
  for (var i = 0; i < itens.length; i++) {
    vetor[i] = itens[i];
  }
  final lista = cobject(6);
  lista.cast<Int64>().elementAt(1).value = itens.length;
  lista.cast<Pointer<Pointer<Uint8>>>().elementAt(2).value = vetor;
  print(post(porta, lista));

  final inteiro = cobject(3)..cast<Int64>().elementAt(1).value = 1 << 40;
  print(post(porta, inteiro));
  print([post(123456789, inteiro), fechar(987654321)]);
}
