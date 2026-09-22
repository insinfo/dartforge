// Convertido de tests/conformance/cases/string_escapes.dart (fixture antigo do corpus de conformidade).
String decorate(String value) {
  return '[' + value + ']';
}
void main() {
  print('first\nsecond');
  print('tab:\tend');
  print('quote: \' " \\');
  print('\x41\u0042\u{43}');
  print('\u{1F600}');
  print('\uD83D\uDE00');
  print('\q');
  print('\0');
  print('cost: \$5');
  print(r'C:\new\test\$name');
  print(r'\');
  print('\é');
  print(decorate('ação\nRust'));
}
