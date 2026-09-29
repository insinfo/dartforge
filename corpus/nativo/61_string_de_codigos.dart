// `String.fromCharCodes` de uma lista do runtime (`createFromCharCodes`,
// `_deCodigos` em `sdk_nativo/core/string_patch.dart`): a lista toda em
// Latin-1 numa cópia só; o resto pelo caminho da VM (duas unidades,
// pares substitutos, erros). A saída tem de ser a da VM.
import 'dart:typed_data';

String erro(void Function() f) {
  try {
    f();
    return 'sem erro';
  } catch (e) {
    return '${e.runtimeType}: $e';
  }
}

void main() {
  final crescivel = <int>[];
  for (final c in 'Host: 127.0.0.1'.codeUnits) {
    crescivel.add(c);
  }
  print(String.fromCharCodes(crescivel));
  print(String.fromCharCodes(crescivel, 6));
  print(String.fromCharCodes(crescivel, 2, 4));
  print(String.fromCharCodes(crescivel, 20));
  print(String.fromCharCodes(crescivel, 3, 3).isEmpty);
  print(String.fromCharCodes(crescivel, 0, 100));
  crescivel.clear();
  print('[${String.fromCharCodes(crescivel)}]');

  final fixa = List<int>.filled(5, 0x41);
  fixa[4] = 0xFF;
  final s = String.fromCharCodes(fixa);
  print('$s ${s.length} ${s.codeUnitAt(4)}');
  print(String.fromCharCodes(const [104, 105]));
  print(identical(String.fromCharCodes([97]), 'a'));
  print(String.fromCharCodes([97]) == 'a');
  print(String.fromCharCodes([0, 1, 255]).codeUnits);

  // Acima de Latin-1: duas unidades e pares substitutos.
  print(String.fromCharCodes([0x48, 0x100, 0x49]).codeUnits);
  print(String.fromCharCodes([0x1F600, 0x41]).length);
  print(String.fromCharCodes([0x41, 0x1F600], 1).codeUnits);

  // Erros como na VM.
  print(erro(() => String.fromCharCodes([65, -1])));
  print(erro(() => String.fromCharCodes([65, 0x110000])));
  print(erro(() => String.fromCharCodes([65, 1 << 62])));
  print(erro(() => String.fromCharCodes([65, 66], -1)));
  print(erro(() => String.fromCharCodes([65, 66], 2, 1)));
  print(erro(() => String.fromCharCodes([65, -1], 0, 1)));

  // Listas tipadas e iteráveis seguem o caminho de sempre.
  print(String.fromCharCodes(Uint8List.fromList([111, 108, 225])));
  print(String.fromCharCodes(Uint16List.fromList([0x3B1, 0x3B2])).codeUnits);
  print(String.fromCharCodes({120, 121}));
  print(String.fromCharCodes([for (var i = 0; i < 5; i++) 48 + i].reversed));
  print(String.fromCharCodes(List<int>.generate(300, (i) => 65 + i % 26), 290));

  // Grande, com a coleta no meio.
  final grande = <int>[];
  for (var i = 0; i < 100000; i++) {
    grande.add(32 + i % 95);
  }
  var total = 0;
  for (var k = 0; k < 50; k++) {
    total += String.fromCharCodes(grande, k, k + 1000).hashCode & 0xF;
  }
  print('${String.fromCharCodes(grande).length} $total');
  print(String.fromCharCode(0x263A).codeUnits);
  print(String.fromCharCodes(List<int>.unmodifiable([79, 75])));
}
