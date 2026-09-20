int baseMark() { print(30); return 30; }
class Base {
  int _value = baseMark();
  Base() { report(); }
  void report() { print(_value); }
  int baseValue() => _value;
}
