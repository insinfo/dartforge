// Coleções do núcleo: List<int>, Map<int, int>, Set<String>.
import 'comum.dart';

int crivo(int n) {
  final primo = List<bool>.filled(n + 1, true);
  var c = 0;
  for (var i = 2; i <= n; i++) {
    if (primo[i]) {
      c++;
      for (var j = i * i; j <= n; j += i) {
        primo[j] = false;
      }
    }
  }
  return c;
}

int listaInt(int n) {
  final l = <int>[];
  for (var i = 0; i < n; i++) {
    l.add(i * 7 % 1000);
  }
  var s = 0;
  for (var r = 0; r < 10; r++) {
    for (var i = 0; i < l.length; i++) {
      s += l[i];
    }
  }
  l.sort();
  return s + l[n ~/ 2];
}

int mapa(int n) {
  final m = <int, int>{};
  for (var i = 0; i < n; i++) {
    m[i * 31 % n] = i;
  }
  var s = 0;
  for (var i = 0; i < n; i++) {
    s += m[i] ?? 0;
  }
  return s;
}

int conjunto(int n) {
  final c = <String>{};
  for (var i = 0; i < n; i++) {
    c.add('k${i % 5000}');
  }
  return c.length;
}

void main() {
  medir('crivo', () => crivo(2000000));
  medir('lista_int', () => listaInt(1000000));
  medir('mapa', () => mapa(500000));
  medir('conjunto_str', () => conjunto(300000));
}
