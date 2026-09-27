import 'dart:async';

// J03: a v2 tira `lixo`, põe `rotulo` (anulável) antes de `valor` e um
// `late` com inicializador; o objeto vivo migra pelo nome dos campos.
class Contador {
  int lixo = 7;
  int valor = 0;
  void subir() => valor += 1;
}

final c = Contador();
var ticks = 0;

String rotulo() => 'v1';

void main() {
  print('main');
  Timer.periodic(Duration(milliseconds: 40), (t) {
    ticks += 1;
    c.subir();
    print('${rotulo()} ${c.valor} ${c.lixo}');
    if (rotulo() != 'v1' || ticks > 2000) t.cancel();
  });
}
