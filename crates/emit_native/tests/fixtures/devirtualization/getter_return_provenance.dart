import 'dart:convert';

int getterCalls = 0;

class EncoderA {
  String convert(String input) => 'A:$input';
}

class EncoderB {
  String convert(String input) => 'B:$input';
}

class Owner {
  dynamic get encoder => EncoderA();
}

class OtherOwner {
  dynamic get encoder => EncoderB();
}

dynamic factoryOwner(bool first) {
  if (first) return Owner();
  return OtherOwner();
}

class CustomCodec {
  dynamic get encoder {
    getterCalls++;
    return EncoderB();
  }
}

class MissingGetter {
  @override
  dynamic noSuchMethod(Invocation invocation) {
    getterCalls++;
    return EncoderA();
  }
}

@pragma('vm:never-inline')
String convertUnknown(dynamic codec, String value) => codec.encoder.convert(value);

void main() {
  dynamic owner = Owner();
  print(owner.encoder.convert('owner'));
  for (final first in [true, false]) {
    dynamic selected = factoryOwner(first);
    print(selected.encoder.convert('phi'));
  }
  print(convertUnknown(CustomCodec(), 'custom'));
  print(convertUnknown(MissingGetter(), 'missing'));
  print('getters:$getterCalls');
  dynamic asciiCodec = const AsciiCodec();
  dynamic utf8Codec = const Utf8Codec();
  dynamic base64Codec = const Base64Codec();
  print(asciiCodec.encoder.convert('A'));
  print(utf8Codec.encoder.convert('é'));
  print(base64Codec.encoder.convert([65, 66]));
}
