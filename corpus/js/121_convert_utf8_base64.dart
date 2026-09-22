// dart:convert: utf8.encode/decode, base64/base64Url, ascii, latin1 e LineSplitter.
import 'dart:convert';

void main() {
  print(utf8.encode('abc'));
  print(utf8.encode('é'));
  print(utf8.encode('ção'));
  print(utf8.encode('😀'));
  print(utf8.encode('').length);
  print(utf8.encode('a😀b').length);
  print(utf8.decode([97, 98, 99]));
  print(utf8.decode([195, 169]));
  print(utf8.decode([240, 159, 152, 128]));
  print(utf8.decode(utf8.encode('round trip é 😀')));
  print(utf8.decode([0xFF], allowMalformed: true) == '�');
  try {
    utf8.decode([0xFF]);
  } catch (e) {
    print('lançou ${e is FormatException}');
  }
  print(utf8.encoder.convert('xy'));
  print(utf8.decoder.convert([120, 121]));
  print(utf8.encode('\u0000').length);
  print(utf8.encode('߿').length);
  print(utf8.encode('ࠀ').length);
  print(utf8.encode('\uFFFF').length);
  print(utf8.encode('\u{10000}').length);
  print(utf8.encode('\u{10FFFF}'));

  print(base64Encode([]));
  print(base64Encode([0]));
  print(base64Encode([0, 0]));
  print(base64Encode([255, 255, 255]));
  print(base64Encode(utf8.encode('Olá, mundo!')));
  print(base64Encode(utf8.encode('a')));
  print(base64Encode(utf8.encode('ab')));
  print(base64Encode(utf8.encode('abc')));
  print(base64.encode([1, 2, 3, 4]));
  print(base64Decode('aGVsbG8='));
  print(utf8.decode(base64Decode('aGVsbG8=')));
  print(utf8.decode(base64.decode('T2zDoSwgbXVuZG8h')));
  print(base64Decode('AAAA'));
  print(base64Decode(''));
  print(base64Decode('AA=='));
  try {
    base64Decode('abc');
  } catch (e) {
    print('lançou ${e is FormatException}');
  }
  print(base64.normalize('AA'));
  print(base64Encode([251, 255]));
  print(base64UrlEncode([251, 255]));
  print(base64Url.encode([251, 255, 254]));
  print(base64Url.decode(base64.normalize('-_8')));
  print(base64Url.decode('-_8='));
  print(base64.decode('+/8='));
  print(base64Encode(utf8.encode('x' * 30)));
  print(base64Encode(List.generate(10, (i) => i * 25)));

  print(ascii.encode('ABC xyz'));
  print(ascii.decode([72, 105]));
  try {
    ascii.encode('é');
  } catch (e) {
    print('lançou ${e is ArgumentError}');
  }
  print(ascii.decode([200], allowInvalid: true) == '�');
  print(latin1.encode('é'));
  print(latin1.encode('ção'));
  print(latin1.decode([233, 231, 227, 111]));
  print(latin1.decode(latin1.encode('àéîõü')));
  print(latin1.encode('ÿ'));
  try {
    latin1.encode('😀');
  } catch (e) {
    print('lançou ${e is ArgumentError}');
  }

  print(LineSplitter.split('a\nb\r\nc\rd').toList());
  print(LineSplitter().convert('um\ndois\n'));
  print(LineSplitter().convert('um\ndois'));
  print(LineSplitter().convert(''));
  print(LineSplitter().convert('\n'));
  print(LineSplitter().convert('\n\n'));
  print(LineSplitter.split('x').toList());
  print(LineSplitter.split('a\nb', 2).toList());
  print(const LineSplitter().convert('1\n2\n3').length);
  print(utf8.decode(base64Decode(base64Encode(utf8.encode('ida e volta')))));
  print(utf8.encode('ab').map((b) => b.toRadixString(16)).join(' '));
  print(String.fromCharCodes(utf8.encode('é')).length);
  print(utf8.decode(latin1.encode('abc')));
  print(latin1.decode(utf8.encode('é')).length);
  print(ascii.encoder.convert('!').first);
  print(json.fuse(utf8).encode({'a': 1}));
  print(utf8.fuse(base64).encode('hi'));
  print(utf8.fuse(base64).decode('aGk='));
}
