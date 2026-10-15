import 'dart:convert';
import 'dart:_foreign_helper' show JS;
void main() {
  final value = JS<int>('!', '1');
  print(JsonUtf8Encoder(null, null, 1).convert({'x': value}).join(','));
}
