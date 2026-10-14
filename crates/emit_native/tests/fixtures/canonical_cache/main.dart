import 'dart:convert';

final class NumberBox {
  final int value;
  const NumberBox(this.value);
}

final class PixelCodec {
  const PixelCodec();
  String convert(String input) => 'pixel:$input';
}

String unknownConvert(dynamic receiver, String input) => receiver.convert(input);

void main() {
  const first = NumberBox(7);
  const same = NumberBox(7);
  const other = NumberBox(11);
  print([first.value, other.value, identical(first, same)]);
  print(ascii.encode('ASCII'));
  print(ascii.decode([65, 66, 67]));
  print(latin1.encode('Ol\u00e1'));
  print(latin1.decode([79, 108, 225]));
  print(utf8.decode(utf8.encode('A\u00e7\u00e3o \u{1f642}')));
  print(base64Encode([0, 1, 2, 253, 254, 255]));
  print(jsonEncode({'mask': 127, 'ok': true}));
  print(unknownConvert(const PixelCodec(), 'unknown'));
  print(Function.apply(const PixelCodec().convert, ['apply']));
  try {
    ascii.encode('\u00e9');
    print('missing ascii error');
  } on ArgumentError {
    print('ascii error');
  }
  print(ascii.encode('warm'));
}
