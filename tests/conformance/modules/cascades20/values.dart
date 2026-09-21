class Counter {
  int _value = 0;
  void add(int amount) { _value += amount; }
  int get value => _value;
  void hidden() { this.._value = 8..add(1); }
}

class Holder {
  Counter child = Counter();
}

Counter make() { print('receiver'); return Counter(); }
Counter? absent() { print('absent'); return null; }
int effect(int value) { print(value); return value; }
int plus(int value) => value + 1;
int other(int value) => value + 1;
