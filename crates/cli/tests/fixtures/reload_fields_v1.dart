import 'dart:async';

// J03: v2 removes `lixo`, adds `rotulo` (nullable) before `valor` and a
// `late` field with an initializer; the live object migrates by field name.
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
