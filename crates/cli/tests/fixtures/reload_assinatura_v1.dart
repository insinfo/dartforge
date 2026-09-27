import 'dart:async';

// J03: na v2, `etiqueta` ganha um parâmetro opcional e `Caixa.vezes` um
// nomeado: as assinaturas nativas mudam, e as funções ganham entradas novas.
String etiqueta(int n) => 'n$n';

class Caixa {
  int base = 1;
  int vezes(int x) => base * x;
}

final c = Caixa();
var ticks = 0;

String rotulo() => 'v1';

void main() {
  print('main');
  Timer.periodic(Duration(milliseconds: 40), (t) {
    ticks += 1;
    print('${rotulo()} ${etiqueta(ticks)} ${c.vezes(ticks)}');
    if (rotulo() != 'v1' || ticks > 2000) t.cancel();
  });
}
