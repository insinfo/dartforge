// Scalar numeric computation: mandelbrot and collatz (double and int without objects).
import 'common.dart';

int mandelbrot(int n) {
  var sum = 0;
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
      sum += i;
    }
  }
  return sum;
}

int collatz(int limit) {
  var longest = 0;
  for (var n = 1; n < limit; n++) {
    var x = n, steps = 0;
    while (x != 1) {
      x = x.isEven ? x >> 1 : 3 * x + 1;
      steps++;
    }
    if (steps > longest) longest = steps;
  }
  return longest;
}

void main() {
  measure('mandelbrot', () => mandelbrot(400));
  measure('collatz', () => collatz(300000));
}
