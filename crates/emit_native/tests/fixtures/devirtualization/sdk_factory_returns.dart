// Ordinary SDK constructors must retain exact behavior through cached linking.
import 'dart:convert';

@pragma('vm:never-inline')
dynamic encoder(bool ascii) => ascii ? AsciiEncoder() : Utf8Encoder();

class OtherEncoder {
  int calls = 0;
  List<int> convert(String text) => [++calls, text.length];
}

@pragma('vm:never-inline')
List<int> unknownConvert(dynamic value) => value.convert('ab') as List<int>;

void main() {
  print(encoder(true).convert('AB'));
  print(encoder(false).convert('é'));
  final custom = OtherEncoder();
  print(unknownConvert(custom));
  print(unknownConvert(custom));
  try {
    encoder(true).convert('é');
  } catch (error) {
    print(error.runtimeType);
  }
}
