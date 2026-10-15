final events = <String>[];

int record(String name, int value) {
  events.add(name);
  return value;
}

int fail(String name) {
  events.add(name);
  throw StateError(name);
}

int choose(int value, int mode) {
  events.add('choose:$mode');
  if (mode == 0) return value + 3;
  if (mode == 1) return value * 7;
  if (mode == 2) throw StateError('mode');
  return value - mode;
}

int recurse(int value, int mode) {
  if (value == 0) return mode;
  return recurse(value - 1, mode);
}

int boolChoice(int value, bool flag) => flag ? value + 1 : value - 1;

enum Mode { add, multiply }

int enumChoice(int value, Mode mode) {
  switch (mode) {
    case Mode.add:
      return value + 3;
    case Mode.multiply:
      return value * 7;
  }
}

void main() {
  print(choose(record('left', 5), 0));
  print(choose(record('left', 5), 1));
  print(choose(record('left', 5), record('right', 4)));
  try {
    choose(record('left', 5), 2);
  } on StateError catch (error) {
    print(error.message);
  } finally {
    events.add('finally');
  }
  try {
    choose(fail('argument'), 1);
  } on StateError catch (error) {
    print(error.message);
  }
  final generic = choose;
  for (final mode in <int>[0, 1, 4]) {
    print(generic(6, mode));
  }
  print(recurse(4, 8));
  print(boolChoice(9, true));
  print(boolChoice(9, false));
  for (var i = 0; i < 2; i++) {
    print(enumChoice(5, Mode.add));
    print(enumChoice(5, Mode.multiply));
  }
  for (var i = 0; i < 0; i++) {
    choose(fail('unreachable'), 0);
  }
  print(events.join('|'));
}
