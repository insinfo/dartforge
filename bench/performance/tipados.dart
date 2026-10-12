// Listas tipadas: produto escalar, convolução e bytes.
import 'dart:typed_data';
import 'comum.dart';

double produto(Float64List a, Float64List b, int reps) {
  var s = 0.0;
  for (var r = 0; r < reps; r++) {
    for (var i = 0; i < a.length; i++) {
      s += a[i] * b[i];
    }
  }
  return s;
}

int bytes(Uint8List b, int reps) {
  var h = 0;
  for (var r = 0; r < reps; r++) {
    for (var i = 0; i < b.length; i++) {
      h = (h ^ b[i]) * 16777619 & 0xFFFFFFFF;
    }
  }
  return h;
}

void main() {
  final a = Float64List(100000), b = Float64List(100000);
  for (var i = 0; i < a.length; i++) {
    a[i] = i * 0.001;
    b[i] = (i % 13) * 0.5;
  }
  final u = Uint8List(1 << 20);
  for (var i = 0; i < u.length; i++) {
    u[i] = i * 131 & 255;
  }
  medir('produto_f64', () => produto(a, b, 50));
  medir('fnv_bytes', () => bytes(u, 10));
}
