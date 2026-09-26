// Acessores de topo por prefixo de import (`g.valor = v`, `??=`, o
// valor da atribuição) e um condicional `dynamic` com ramos de
// representações diferentes (`int` e `dynamic`), dois casos do package:intl.
import 'g.dart' as g;

String? definir(String? n) => g.valor = n;

dynamic piso(dynamic n) => (n is num) ? n.floor() : n ~/ 1;

void main() {
  print(g.valor);
  print(definir('x'));
  print(g.valor);
  g.valor ??= 'y';
  print(g.valor);
  g.contador += 5;
  g.contador++;
  print(g.contador);
  print(piso(3.7));
  print(piso(-2.5));
}
