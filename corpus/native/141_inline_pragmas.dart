// Inlining requests preserve evaluation order, exceptions and async results.
import 'dart:async';

int effects = 0;
int argument() => ++effects;

@pragma('vm:prefer-inline')
int preferred(int first, int unused) => first + 1;

@pragma('vm:prefer-inline')
@pragma('vm:never-inline')
int forbidden(int value) {
  if (value < 0) throw StateError('negative');
  return value * 2;
}

class Counter {
  @pragma('vm:never-inline')
  Counter(this.value);

  final int value;

  @pragma('vm:prefer-inline')
  int add(int amount) => value + amount;

  @pragma('vm:never-inline')
  int get twice => value * 2;
}

@pragma('vm:never-inline')
Future<int> asynchronous(int value) async {
  await Future<void>.value();
  return forbidden(value);
}

Future<void> main() async {
  print(preferred(argument(), argument()));
  print(effects);
  final counter = Counter(7);
  print(counter.add(3));
  print(counter.twice);
  print(await asynchronous(5));
  try {
    forbidden(-1);
  } on StateError {
    print('caught');
  } finally {
    print('finally');
  }
}
