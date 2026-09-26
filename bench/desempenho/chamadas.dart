// Despacho virtual, closures e recursão.
import 'comum.dart';

abstract class Forma {
  double area();
}

class Quadrado extends Forma {
  final double l;
  Quadrado(this.l);
  @override
  double area() => l * l;
}

class Circulo extends Forma {
  final double r;
  Circulo(this.r);
  @override
  double area() => 3.14159 * r * r;
}

class Ret extends Forma {
  final double a, b;
  Ret(this.a, this.b);
  @override
  double area() => a * b;
}

double formas(int n) {
  final fs = <Forma>[for (var i = 0; i < 1000; i++) i % 3 == 0 ? Quadrado(i * 0.1) : i % 3 == 1 ? Circulo(i * 0.1) : Ret(i * 0.1, 2)];
  var s = 0.0;
  for (var r = 0; r < n; r++) {
    for (final f in fs) {
      s += f.area();
    }
  }
  return s;
}

int closures(int n) {
  int Function(int) somador(int k) => (x) => x + k;
  final fs = [for (var k = 0; k < 8; k++) somador(k)];
  var s = 0;
  for (var i = 0; i < n; i++) {
    s = fs[i & 7](s) & 0xFFFFFF;
  }
  return s;
}

int fib(int n) => n < 2 ? n : fib(n - 1) + fib(n - 2);

void main() {
  medir('formas', () => formas(2000));
  medir('closures', () => closures(5000000));
  medir('fib', () => fib(30));
}
