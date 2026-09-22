import 'base.dart';
class Child extends Base {
  int _value = 7;
  int childValue() { return this._value; }
  int value() { return this._value + 1; }
}
