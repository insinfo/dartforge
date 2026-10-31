// Production forms of `docs/JS-PRODUCAO-VELOCIDADE.md` §11: constants
// evaluated once per site, `for`-in through the Dart iterator, the `S? as S`
// check that only tests `null`, `is C` by the prototype chain, `is num`/`is
// int` and `identical(x, literal)` without the runtime, and the native sort.
// Every answer is the VM's.
import 'dart:collection';
import 'dart:convert';

class P {
  final int x, y;
  const P(this.x, this.y);
  @override
  String toString() => 'P($x, $y)';
}

class Box<T> {
  final T? value;
  const Box(this.value);
  T get unwrapped => value as T;
}

enum Color { red, green }

const top = P(3, 4);

List<T> emptyOf<T>() => const [];

P site() => const P(1, 2);

Object constantOf(int i) {
  switch (i % 5) {
    case 0:
      return const [1, 2];
    case 1:
      return const {'a': 1};
    case 2:
      return const {1, 2};
    case 3:
      return const (1, 'b');
    default:
      return #sym;
  }
}

/// An iterable whose iterator counts the calls, to show the protocol.
class Counted extends IterableBase<int> {
  final List<String> log;
  final int n;
  Counted(this.log, this.n);
  @override
  Iterator<int> get iterator {
    log.add('iterator');
    return _CountedIterator(log, n);
  }
}

class _CountedIterator implements Iterator<int> {
  final List<String> log;
  final int n;
  int i = -1;
  _CountedIterator(this.log, this.n);
  @override
  bool moveNext() {
    log.add('moveNext');
    return ++i < n;
  }

  @override
  int get current {
    log.add('current $i');
    return i;
  }
}

/// A list that is not a JavaScript array.
class Wrapped extends ListBase<int> {
  final List<int> inner;
  Wrapped(this.inner);
  @override
  int get length => inner.length;
  @override
  set length(int n) => inner.length = n;
  @override
  int operator [](int i) => inner[i];
  @override
  void operator []=(int i, int v) => inner[i] = v;
  @override
  void add(int v) => inner.add(v);
}

void constants() {
  // The same canonical object at every site and every evaluation.
  final seen = <Object>[];
  for (var i = 0; i < 3; i++) {
    seen.add(const P(1, 2));
    seen.add(site());
  }
  print(seen.every((p) => identical(p, seen.first)));
  print(identical(top, const P(3, 4)));
  for (var i = 0; i < 10; i++) {
    final a = constantOf(i), b = constantOf(i);
    if (!identical(a, b)) print('not identical: $a');
  }
  print([for (var i = 0; i < 5; i++) constantOf(i)]);
  print(emptyOf<int>().runtimeType);
  print(identical(emptyOf<int>(), emptyOf<String>()));
  // A constant that reads a local constant: one value per function.
  int first() {
    const k = 3;
    return const [k, 2][0];
  }

  int second() {
    const k = 4;
    return const [k, 2][0];
  }

  print('${first()} ${second()} ${first()}');
  print(identical(Color.values, Color.values));
  print(const [Color.red, Color.green] == Color.values);
  // The SDK getters behind `utf8` and `json`.
  for (var i = 0; i < 3; i++) {
    print('${utf8.encode('é$i')} ${utf8.decode(utf8.encode('ç$i'))} ${jsonEncode({'k': i})}');
  }
}

void iteration() {
  final log = <String>[];
  for (final v in Counted(log, 2)) {
    log.add('body $v');
  }
  print(log.join(', '));
  final m = <String, int>{'a': 1, 'b': 2, 'c': 3};
  var s = '';
  for (final e in m.entries) {
    s += '${e.key}=${e.value} ';
  }
  for (final k in m.keys) {
    if (k == 'b') continue;
    s += k;
  }
  outer:
  for (final v in m.values) {
    for (final w in {v, v + 1}) {
      if (w == 3) break outer;
      s += ' $w';
    }
  }
  print(s);
  // Patterns and an assigned variable as the loop target.
  for (final (a, b) in [(1, 'x'), (2, 'y')].where((r) => r.$1 > 0)) {
    print('$a$b');
  }
  for (final MapEntry(:key, :value) in m.entries) {
    print('$key$value');
  }
  int last = -1;
  for (last in Iterable.generate(3)) {}
  print(last);
  // A list that is not an array, and a generic iterable of a type variable.
  print([for (final v in Wrapped([5, 6, 7])) v * 2]);
  print(sumAll<int>([1, 2, 3].map((x) => x)));
  // Modifying while iterating: the Dart iterator throws.
  final grow = Wrapped([1, 2]);
  try {
    for (final v in grow) {
      if (grow.length < 5) grow.add(v);
    }
  } on ConcurrentModificationError catch (e) {
    print('CME ${e.runtimeType}');
  }
  final set = {1, 2, 3};
  try {
    for (final v in set) {
      set.add(v + 10);
    }
  } on ConcurrentModificationError {
    print('set CME');
  }
  try {
    for (final v in m.keys) {
      m['z$v'] = 0;
    }
  } on ConcurrentModificationError {
    print('map CME');
  }
  // Exceptions thrown by `moveNext` and `current` reach the caller.
  try {
    for (final v in Iterable<int>.generate(3, (i) => i == 1 ? throw 'bad $i' : i)) {
      print('got $v');
    }
  } catch (e) {
    print('caught $e');
  }
  // A dynamic iterable is cast to `Iterable` first.
  dynamic d = [1, 2];
  for (int v in d) {
    print(v + 1);
  }
}

int sumAll<T extends num>(Iterable<T> xs) {
  var s = 0;
  for (final x in xs) {
    s += x.toInt();
  }
  return s;
}

void casts() {
  print(const Box<int>(1).unwrapped);
  print(const Box<int?>(null).unwrapped);
  try {
    print(const Box<int>(null).unwrapped);
  } on TypeError {
    print('TypeError for null');
  }
  int? n = DateTime.now().year > 0 ? 7 : null;
  print((n as int) + 1);
  String? f() => null;
  try {
    print(f() as String);
  } on TypeError {
    print('TypeError for f()');
  }
  var calls = 0;
  int? g() {
    calls++;
    return 5;
  }

  print('${g() as int} $calls');
  final it = [1, 2, 3].iterator;
  while (it.moveNext()) {
    print(it.current);
  }
}

class Base {}

class Sub extends Base {}

class SubSub extends Sub {}

class Implemented {}

class Implementer implements Implemented {}

mixin class Mixed {}

class WithMixed extends Base with Mixed {}

class Failure implements Exception {
  final String why;
  Failure(this.why);
}

class Point3 {
  final int x, y, z;
  Point3(this.x, this.y, this.z);
  @override
  bool operator ==(Object other) => other is Point3 && other.x == x && other.y == y && other.z == z;
  @override
  int get hashCode => Object.hash(x, y, z);
}

String describe(Object? o) => switch (o) {
      SubSub() => 'SubSub',
      Sub() => 'Sub',
      Base() => 'Base',
      Implemented() => 'Implemented',
      Mixed() => 'Mixed',
      null => 'null',
      _ => 'other ${o.runtimeType}',
    };

void typeTests() {
  final values = <Object?>[Base(), Sub(), SubSub(), Implementer(), WithMixed(), Mixed(), null, 3, 'x'];
  for (final v in values) {
    print('${describe(v)} ${v is Base} ${v is Sub} ${v is Implemented} ${v is Mixed} ${v is! SubSub}');
  }
  print(Point3(1, 2, 3) == Point3(1, 2, 3));
  print(Point3(1, 2, 3) == Point3(1, 2, 4));
  // ignore: unrelated_type_equality_checks
  print(Point3(1, 2, 3) == 'p');
  print({Point3(1, 2, 3), Point3(1, 2, 3)}.length);
  try {
    throw Failure('because');
  } on Failure catch (e) {
    print('caught ${e.why}');
  }
  try {
    try {
      throw Sub();
    } on SubSub {
      print('wrong');
    }
  } on Base catch (e) {
    print('outer ${e is Sub}');
  }
  Object o = SubSub();
  if (o is Sub) {
    print('promoted ${o.runtimeType}');
  }
  // Number tests and `identical` with a literal.
  // (Only values whose answers the web shares: there `1 is double` holds.)
  final numbers = <Object?>[2.5, double.nan, -1.5, '1', true, null];
  for (final v in numbers) {
    print('${v is num} ${v is int} ${v is double} ${identical(v, true)} ${identical(null, v)} '
        '${identical(v, 1)} ${identical('1', v)} ${identical(v, 2.5)}');
  }
  for (final v in <Object>[1, 9007199254740991, -7]) {
    print('${v is num} ${v is int} ${identical(v, 1)} ${identical(-7, v)}');
  }
  var count = 0;
  Object? next() => numbers[count++];
  print('${identical(next(), 1)} ${identical(next(), null)} ${next() is int} $count');
  print(identical(null, null));
}

class Ranked implements Comparable<Ranked> {
  final int rank;
  final String name;
  Ranked(this.rank, this.name);
  @override
  int compareTo(Ranked other) => rank - other.rank;
  @override
  String toString() => '$name$rank';
}

/// A number as every platform prints it (the web prints `0.0` as `0`).
String show(num d) => d == 0 ? (d.isNegative ? '-0' : '+0') : d.toStringAsExponential(2);

void sorting() {
  final ints = [5, -3, 9007199254740991, 0, 42, -9007199254740991, 7, 7, 1];
  print(ints..sort());
  final doubles = <double>[3.5, -0.0, double.nan, 0.0, double.infinity, -1e-300, double.negativeInfinity, -0.0, 0.0];
  doubles.sort();
  print(doubles.map(show).join(' '));
  final nums = <num>[3, 2.5, -1, 0.0, -0.0, 1e21, 2];
  print((nums..sort()).map(show).join(' '));
  final strings = ['b', 'B', 'a', '', 'é', 'é', '😀', '￿', 'aa', 'a'];
  print((strings..sort()).map((s) => s.codeUnits.join('.')).join(' '));
  print((<int>[]..sort()).length);
  print(([9]..sort()));
  print((List<int>.filled(3, 2, growable: false)..sort()));
  final ranked = [Ranked(2, 'b'), Ranked(1, 'a'), Ranked(2, 'c'), Ranked(0, 'z')];
  ranked.sort();
  print(ranked.map((r) => r.rank).join(' '));
  // (`[1, 'a']` is left out: `num.compareTo` of a `String` does not throw
  // in DartForge's JavaScript, with or without the native sort.)
  for (final bad in <List<Object?>>[
    [null, 1],
    [true, false],
  ]) {
    try {
      bad.sort();
      print('sorted $bad');
    } catch (e) {
      print('cannot sort ${bad.length}: ${e is TypeError || e is NoSuchMethodError}');
    }
  }
  try {
    (const [3, 1, 2]).sort();
  } on UnsupportedError {
    print('const list is unmodifiable');
  }
}

void main() {
  constants();
  iteration();
  casts();
  typeTests();
  sorting();
}
