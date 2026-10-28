// The production profile converts `print` arguments, interpolations and
// `toString()` of non-nullable strings, numbers and booleans in place, and
// writes `i % k` with a positive literal `k` inline, and iterates a list
// literal of a `for-in` as a bare array. Every line here must match the VM.

class Point {
  final int x;
  Point(this.x);
  @override
  String toString() => 'Point($x)';
}

String describe(Object? o) => 'value: $o';

void main() {
  print('plain');
  var i = 41;
  print(i);
  print(i + 1);
  print(-i);
  double d = 2.5;
  print(d);
  print(-0.0);
  print(1e21);
  print(0.1 + 0.2);
  num n = 7;
  print(n);
  num m = 7.5;
  print(m);
  print(true);
  bool b = i > 40;
  print(b);
  print(!b);
  print('$b');
  print('b=$b, !b=${!b}');
  print('${i > 1}${i < 1}');
  int? maybe = i > 0 ? i : null;
  print(maybe);
  int? none = i < 0 ? i : null;
  print(none);
  print(null);
  print(Point(3));
  print(describe(Point(4)));
  print(describe(i));
  print([1, 2, 3]);
  print('i=$i d=$d n=$n m=$m');
  print(i.toString());
  print(d.toString() + n.toString());
  print(b.toString());
  print('s'.toString());
  print((i * 2).toString().length);
  print((-0.0).toString());
  Object o = i;
  print(o.toString());

  // Euclidean modulo with a literal divisor.
  for (var k = -7; k <= 7; k++) {
    print('$k % 3 = ${k % 3}, $k % 1 = ${k % 1}, $k % 2 = ${k % 2}');
  }
  var big = 9007199254740991;
  print(big % 10);
  print(-big % 7);
  print((-6) % 3);
  print(-6 % 3 == 0);
  print('${-6 % 3}');
  var zero = -4 % 2;
  print(zero);
  print(1 / (zero + 0.0) > 0);
  print(2147483647 % 2147483647);
  print(-1 % 2147483647);
  int q = -9;
  q %= 4;
  print(q);

  // A list literal iterated directly by `for-in`.
  for (final v in [3, -1, 4]) {
    print(v);
  }
  for (num v in [1, 2.5]) {
    print(v);
  }
  var total = 0;
  for (var s in ['a', 'bb', 'ccc']) {
    total += s.length;
  }
  print(total);
}
