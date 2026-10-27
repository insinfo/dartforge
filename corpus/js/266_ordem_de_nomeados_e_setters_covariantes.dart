// Named arguments written before positional ones are evaluated in source
// order. Named parameters with Object.prototype names (constructor,
// toString…) are only taken when passed. A final static field and a static
// setter can share a name. A mixin's setter under a covariant supertype
// setter checks its value.
List<Object?> log = [];

T note<T>(T x) {
  log.add(x);
  return x;
}

void f(int x, String y, {bool z = false, double w = 0}) {}

String names({String constructor = 'c', String toString = 'S'}) =>
    '$constructor$toString';

class Counter {
  static int writes = 0;
}

class A {
  static const int field = 0;
  static set field(int value) => Counter.writes += value;
}

class Super<T> {
  T? field;
}

mixin Mixin {
  int? field;
}

class Class = Super<int> with Mixin;

void main() {
  f(note(1), z: note(true), note('2'), w: note(3.5));
  dynamic g = f;
  g(note(4), w: note(5.5), note('6'));
  print(log);
  print([names(), names(constructor: 'X'), names(toString: 'Y')]);
  A.field = 5;
  print([A.field, Counter.writes]);
  Super<Object> s = Class();
  s.field = 3;
  try {
    s.field = 'text';
    print('accepted');
  } on TypeError {
    print('TypeError');
  }
  print(s.field);
}
