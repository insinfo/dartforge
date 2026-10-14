String value(num x) {
  if (x.isNaN) return 'nan';
  if (x.isInfinite) return x.isNegative ? '-infinity' : 'infinity';
  if (x == 0) return x.isNegative ? '-zero' : 'zero';
  return x.toDouble().toStringAsFixed(3);
}

final events = <String>[];
class JSNumber {
  final int number;
  JSNumber(this.number);
  bool operator <(JSNumber other) { events.add('lt'); return number < other.number; }
  bool operator >(JSNumber other) { events.add('gt'); return number > other.number; }
  num operator %(num other) { events.add('mod'); return 42; }
}

num left() { events.add('left'); return -7.25; }
num right() { events.add('right'); return -3.0; }

void main() {
  final numbers = <double>[-0.0, 0.0, 1.0, -1.0, 7.25, -7.25, double.nan, double.infinity, -double.infinity, 9007199254740992.0];
  final divisors = <double>[3.0, -3.0, 0.0, -0.0, double.infinity, -double.infinity, double.nan];
  for (final a in numbers) {
    for (final b in divisors) {
      print('${value(a)}:${value(b)}=${value(a % b)}:${a < b}:${a > b}:${a <= b}:${a >= b}:${a.compareTo(b)}');
    }
  }
  for (final a in [-100, -1, 0, 1, 100, 4503599627370495]) {
    print('int=$a:${a % 3}:${a % -3}');
  }
  print('ordered=${value(left() % right())}:$events');
  events.clear();
  final a = JSNumber(1), b = JSNumber(2);
  print('custom=${a < b}:${a > b}:${a % 3}:$events');
  dynamic dynamicLeft = 1.0;
  for (final bad in <Object?>[null, JSNumber(2), '2']) {
    try {
      dynamicLeft % bad;
      print('bad=accepted');
    } catch (error) {
      print('bad=${error is TypeError}');
    }
  }
}
