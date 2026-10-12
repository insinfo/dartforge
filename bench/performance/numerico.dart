// Cálculo numérico escalar: mandelbrot e n-corpos (double e int sem objetos).
import 'comum.dart';

int mandelbrot(int n) {
  var soma = 0;
  for (var y = 0; y < n; y++) {
    final ci = 2.0 * y / n - 1.0;
    for (var x = 0; x < n; x++) {
      final cr = 2.0 * x / n - 1.5;
      var zr = 0.0, zi = 0.0;
      var i = 0;
      while (i < 50 && zr * zr + zi * zi < 4.0) {
        final t = zr * zr - zi * zi + cr;
        zi = 2.0 * zr * zi + ci;
        zr = t;
        i++;
      }
      soma += i;
    }
  }
  return soma;
}

int colatz(int limite) {
  var maior = 0;
  for (var n = 1; n < limite; n++) {
    var x = n, passos = 0;
    while (x != 1) {
      x = x.isEven ? x >> 1 : 3 * x + 1;
      passos++;
    }
    if (passos > maior) maior = passos;
  }
  return maior;
}

void main() {
  medir('mandelbrot', () => mandelbrot(400));
  medir('collatz', () => colatz(300000));
}
