extension IntMath on int {
  int triple() { return this * 3; }
  int add(int other) { return this + other; }
}
extension Text on String { String bracket() { return '[' + this + ']'; } }
extension Flags on bool { bool inverse() { return !this; } }
int receiver() { print('receiver'); return 4; }
int argument() { print('argument'); return 2; }
void main() {
  print((5).triple()); print('ação'.bracket()); print(true.inverse());
  print(receiver().add(argument()));
  int? value = 3;
  if (value != null) { print(value.triple()); }
}
