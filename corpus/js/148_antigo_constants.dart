// Convertido de tests/conformance/cases/constants.dart (fixture antigo do corpus de conformidade).
int effect() { print('effect'); return 6; }
class Constants {
  int value = (2 + 3) * 4;
  int getValue() { return this.value + (8 - 2); }
}
void main() {
  print((2 + 3) * (4 - 1));
  print(2147483647 + 1);
  print(-(-0));
  print('ação ' + '😀');
  print(('ab' + 'cd') == 'abcd');
  print(3 ?? effect());
  print(null ?? effect());
  print(false && (effect() == 6));
  print(true || (effect() == 6));
  print(Constants().getValue());
}
