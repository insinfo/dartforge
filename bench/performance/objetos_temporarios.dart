// Objetos que não escapam: um compilador com análise de escape e
// substituição escalar não precisa alocá-los.
import 'comum.dart';

class Ponto {
  final double x;
  final double y;
  const Ponto(this.x, this.y);
  Ponto operator +(Ponto o) => Ponto(x + o.x, y + o.y);
  Ponto escala(double k) => Ponto(x * k, y * k);
  double get norma2 => x * x + y * y;
}

double soma(double a, double b) {
  final p = Ponto(a, b);
  return p.x + p.y;
}

double pontos(int n) {
  var acc = const Ponto(0, 0);
  for (var i = 0; i < n; i++) {
    acc = acc + Ponto(i.toDouble(), 1.0).escala(0.5);
  }
  return acc.norma2;
}

double somas(int n) {
  var s = 0.0;
  for (var i = 0; i < n; i++) {
    s += soma(i.toDouble(), 2.0);
  }
  return s;
}

void main() {
  medir('pontos', () => pontos(2000000));
  medir('soma_ponto', () => somas(5000000));
}
