// Objects that do not escape: a compiler with escape analysis and scalar
// replacement does not need to allocate them.
import 'common.dart';

class Point {
  final double x;
  final double y;
  const Point(this.x, this.y);
  Point operator +(Point o) => Point(x + o.x, y + o.y);
  Point scale(double k) => Point(x * k, y * k);
  double get norm2 => x * x + y * y;
}

double sum(double a, double b) {
  final p = Point(a, b);
  return p.x + p.y;
}

double points(int n) {
  var acc = const Point(0, 0);
  for (var i = 0; i < n; i++) {
    acc = acc + Point(i.toDouble(), 1.0).scale(0.5);
  }
  return acc.norm2;
}

double sums(int n) {
  var s = 0.0;
  for (var i = 0; i < n; i++) {
    s += sum(i.toDouble(), 2.0);
  }
  return s;
}

void main() {
  measure('points', () => points(2000000));
  measure('point_sum', () => sums(5000000));
}
