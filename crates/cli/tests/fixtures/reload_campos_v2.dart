import 'dart:async';

// J03: a v2 tira `lixo`, põe `rotulo` (anulável) antes de `valor` e um
// `late` com inicializador; o objeto vivo migra pelo nome dos campos.
class Contador {
  String? rotulo;
  int valor = 0;
  late int dobro = valor * 2;
  void subir() => valor += 1;
}

final c = Contador();
var ticks = 0;

String rotulo() => 'v2';

void main() {
  print('main');
  Timer.periodic(Duration(milliseconds: 40), (t) {
    ticks += 1;
    c.subir();
    print('${rotulo()} ${c.valor} ${c.rotulo} ${c.dobro == c.valor * 2}');
    if (rotulo() != 'v1' || ticks > 2000) t.cancel();
  });
}
