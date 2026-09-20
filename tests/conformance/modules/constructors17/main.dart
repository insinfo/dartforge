import 'base.dart';
int mark(int value) { print(value); return value; }
String text(String value) { print(value); return value; }
class Usuario extends Base {
  String _name;
  String email;
  int _value = mark(20);
  int? optional;
  Usuario(this._name, this.email) { _name = _name + '!'; }
  @override
  void report() { print(_name); }
  String describe(String _name) => _name + this._name;
  int shadow(int _value) {
    _value += 2;
    { int _value = 100; print(_value); }
    return _value;
  }
  int get value => _value;
  void increase() { _value = _value + 1; }
  int run() { increase(); return value; }
}
class Counter {
  int value = 3;
  int run() { value += 1; return value; }
}
class Mutable {
  int value = mark(5);
  Mutable(this.value) { value += 1; }
}
class Typed {
  int value = 1;
  Typed(int value) { this.value = value; }
}
class Fixed {
  final int value;
  Fixed(this.value) { if (value > 0) { return; } print(0); }
}
void main() {
  var user = Usuario(text('a'), text('b'));
  print(user.describe('local:'));
  print(user.email);
  print(user.shadow(5));
  print(user.run());
  print(user.baseValue());
  print(user.optional);
  print(Counter().run());
  print(Mutable(9).value);
  print(Typed(8).value);
  print(Fixed(4).value);
}
