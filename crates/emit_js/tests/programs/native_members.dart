// The production profile's direct JavaScript forms (docs/JS-PRODUCAO-VELOCIDADE.md):
// `length`/`isEmpty`/`isNotEmpty` of the indexable primitives, `for-in` over a
// list by index, trusted index writes, `toDouble`/`toInt` of numbers, masks
// without `>>> 0`, `const` constructors without the leading `null`s and
// `async` bodies without `await`. Every line must match the VM.
import 'dart:async';
import 'dart:typed_data';

final log = <String>[];

int at(int i) {
  log.add('index $i');
  return i;
}

int value(int v) {
  log.add('value $v');
  return v;
}

class P {
  final double x;
  final double y;
  const P(this.x, this.y);
  const P.swapped(double a, double b)
      : y = a,
        x = b;
  double get sum => x + y;
}

class Q {
  final int a;
  final String b;
  const Q(this.b, this.a);
}

class Counter {
  int n = 0;
  Future<int> bump() async {
    n++;
    return n;
  }

  Future<int> fail() async {
    throw StateError('no $n');
  }
}

Future<int> plain(int i) async => i & 7;

Future<void> nothing() async {}

@pragma('vm:never-inline')
bool odd(int x) => !x.isEven;

@pragma('vm:never-inline')
bool notEmpty(List<int> l) => !l.isEmpty;

@pragma('vm:never-inline')
bool emptyText(String s) => !s.isNotEmpty;

Future<void> main() async {
  print([odd(3), odd(4), notEmpty([]), notEmpty([1]), emptyText(''), emptyText('a')]);

  final list = <int>[1, 2, 3];
  final bytes = Uint8List(4);
  final text = 'hello';
  print([list.length, bytes.length, text.length, list.isEmpty, bytes.isNotEmpty, text.isEmpty]);

  var sum = 0;
  for (final v in list) {
    sum += v;
  }
  for (final b in bytes) {
    sum += b;
  }
  final nested = [
    [1, 2],
    [3]
  ];
  for (final inner in nested) {
    for (final v in inner) {
      if (v == 2) continue;
      sum += v * 10;
    }
  }
  final closures = <int Function()>[];
  for (final v in list) {
    closures.add(() => v);
  }
  print([sum, for (final c in closures) c()]);

  list[at(0)] = value(10);
  bytes[1] = 300;
  bytes[at(2)] = value(-1);
  print([list, bytes, log]);
  log.clear();

  const fixed = [1, 2, 3];
  try {
    fixed[at(0)] = value(5);
  } on UnsupportedError {
    print('unsupported $log');
  }
  log.clear();
  final growable = List<int>.filled(2, 0);
  growable[1] = 7;
  print(growable);

  final i = 7;
  final d = 2.5;
  num n = 3;
  print([i.toDouble() + 0.5, d.toDouble(), n.toDouble() + 0.25, i.toInt(), -1 & 255, (-5) & 0x7fffffff, 6 & 3]);

  const a = P(1.5, 2.5);
  const b = P.swapped(2.5, 1.5);
  print([identical(a, b), a.sum + 0.5, b.x, identical(const Q('x', 1), const Q('x', 1))]);
  var acc = const P(0, 0);
  for (var k = 0; k < 3; k++) {
    acc = P(acc.x + k * 0.5, acc.y + 1.25);
  }
  print([acc.x, acc.y]);

  final c = Counter();
  print([await c.bump(), await c.bump(), await plain(13)]);
  await nothing();
  try {
    await c.fail();
  } on StateError catch (e) {
    print('caught ${e.message}');
  }
  final order = <String>[];
  final f = c.bump().then((v) => order.add('then $v'));
  order.add('after call');
  await f;
  print(order);
  await runZoned(() async {
    print('zoned ${await plain(9)}');
  });
}
