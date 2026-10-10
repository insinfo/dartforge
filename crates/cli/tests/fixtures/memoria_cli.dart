import 'dart:ffi';

@Native<Int8 Function(Int64)>(symbol: 'dartforge_arc_verificar_abi')
external int memoriaAtiva(int versao);

void main(List<String> args) {
  print(memoriaAtiva(1));
  print(args.join('|'));
}
