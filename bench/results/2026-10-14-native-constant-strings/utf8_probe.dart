import 'dart:convert';
void main() {
  for (final text in ['ascii', 'ação', '😀', 'A\u0000Z', 'aé😀']) {
    print(utf8.decode(utf8.encode(text)) == text);
  }
  final emoji = utf8.decode([0xf0, 0x9f, 0x98, 0x80]);
  print(emoji.length);
  print(emoji.codeUnitAt(0));
  print(emoji.codeUnitAt(1));
  final malformed = utf8.decode([0xff, 0xc0, 0xaf, 0xed, 0xa0, 0x80, 0xf4, 0x90, 0x80, 0x80], allowMalformed: true);
  print(malformed.length);
  for (var i = 0; i < malformed.length; i++) { print(malformed.codeUnitAt(i)); }
  final json = '{"text":"ação😀","items":[1,true,null]}';
  print(jsonEncode(jsonDecode(utf8.decode(utf8.encode(json)))));
}
