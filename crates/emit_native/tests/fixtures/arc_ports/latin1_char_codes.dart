import 'dart:convert';
import 'dart:typed_data';

String describe(String text) {
  var sum = 0;
  for (var i = 0; i < text.length; i++) {
    sum = (sum * 31 + text.codeUnitAt(i)) & 0x3fffffff;
  }
  return '${text.length}:$sum';
}

void main() {
  final codes = List<int>.generate(1024, (i) => i & 255);
  for (final source in [codes, List<int>.unmodifiable(codes)]) {
    print(describe(String.fromCharCodes(source, 13, 1000)));
    print(describe(String.fromCharCodes(source, 17, 17)));
  }
  print(describe(String.fromCharCodes([0, 65, 128, 255])));
  print(describe(String.fromCharCodes(Uint8List.fromList(codes), 13, 1000)));
  print(describe(String.fromCharCodes(Uint16List.fromList([0, 255, 256, 0xd800, 0xdc00]))));
  print(describe(String.fromCharCodes([0x10000, 0x10ffff])));
  for (final source in [[-1], [0x110000], [0x7fffffffffffffff]]) {
    try {
      String.fromCharCodes(source);
      print('unexpected success');
    } on ArgumentError {
      print('invalid code');
    }
  }
  for (final bounds in [[-1, 1], [2, 1]]) {
    try {
      String.fromCharCodes(codes, bounds[0], bounds[1]);
      print('unexpected range success');
    } on RangeError {
      print('invalid range');
    }
  }
  print(jsonDecode(jsonEncode({'message': 'Latin1 \u0000\u0080\u00ff', 'items': [1, 2, 3]})));
}
