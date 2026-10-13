// Virtual dispatch, closures and recursion.
import 'common.dart';

abstract class Shape {
  double area();
}

class Square extends Shape {
  final double l;
  Square(this.l);
  @override
  double area() => l * l;
}

class Circle extends Shape {
  final double r;
  Circle(this.r);
  @override
  double area() => 3.14159 * r * r;
}

class Rect extends Shape {
  final double a, b;
  Rect(this.a, this.b);
  @override
  double area() => a * b;
}

double shapes(int n) {
  final fs = <Shape>[for (var i = 0; i < 1000; i++) i % 3 == 0 ? Square(i * 0.1) : i % 3 == 1 ? Circle(i * 0.1) : Rect(i * 0.1, 2)];
  var s = 0.0;
  for (var r = 0; r < n; r++) {
    for (final f in fs) {
      s += f.area();
    }
  }
  return s;
}

int closures(int n) {
  int Function(int) adder(int k) => (x) => x + k;
  final fs = [for (var k = 0; k < 8; k++) adder(k)];
  var s = 0;
  for (var i = 0; i < n; i++) {
    s = fs[i & 7](s) & 0xFFFFFF;
  }
  return s;
}

int fib(int n) => n < 2 ? n : fib(n - 1) + fib(n - 2);

void main() {
  measure('shapes', () => shapes(2000));
  measure('closures', () => closures(5000000));
  measure('fib', () => fib(30));
}
