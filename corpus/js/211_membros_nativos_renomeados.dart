// Membros nativos renomeados por `@JSName` (regra `_isSymbolizedMember` do DDC):
// em `dart:_native_typed_data`, `lengthInBytes` é `@JSName('byteLength')`,
// `offsetInBytes` é `@JSName('byteOffset')` e `elementSizeInBytes` é
// `@JSName('BYTES_PER_ELEMENT')` — o nome Dart não existe no objeto JS, então o
// acesso tem de ser pelo símbolo `dartx`, e não por propriedade direta.
import 'dart:typed_data';

int tamanho(TypedData d) => d.lengthInBytes;

void main() {
  final bytes = Uint8List(8);
  print(bytes.lengthInBytes);
  print(bytes.offsetInBytes);
  print(bytes.elementSizeInBytes);
  print(bytes.buffer.lengthInBytes);
  print(tamanho(bytes));

  final dezesseis = Uint16List(4);
  print(dezesseis.lengthInBytes);
  print(dezesseis.elementSizeInBytes);

  final vista = Uint16List.view(bytes.buffer, 2, 3);
  print(vista.offsetInBytes);
  print(vista.lengthInBytes);
  print(vista.elementSizeInBytes);
  print(vista.buffer.lengthInBytes);

  final doubles = Float64List(2);
  print(doubles.lengthInBytes);
  print(doubles.elementSizeInBytes);

  // Membros com corpo Dart na mesma classe continuam a funcionar.
  final dados = ByteData.view(bytes.buffer);
  dados.setUint16(0, 0x1234);
  print(dados.getUint16(0));
  dados.setInt32(4, -7, Endian.little);
  print(dados.getInt32(4, Endian.little));
  print(dados.lengthInBytes);
  print(dados.offsetInBytes);

  // Receptor dinâmico e interface `TypedData`.
  dynamic d = bytes;
  print(d.lengthInBytes);
  TypedData t = vista;
  print(t.offsetInBytes);
  print([bytes, dezesseis, doubles].map(tamanho).toList());
}
