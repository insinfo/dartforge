import 'dart:async';

// J03: v2 inserts a class before this one (in name order); the live object
// still belongs to this class, with its state and the getter's new body.
class Zeta {
  int valor = 0;
  void subir() => valor += 1;
  String get nome => 'z';
}

final z = Zeta();
var ticks = 0;

String rotulo() => 'v1';

void main() {
  print('main');
  Timer.periodic(Duration(milliseconds: 40), (t) {
    ticks += 1;
    z.subir();
    print('${rotulo()} ${z.valor} ${z.nome}');
    if (rotulo() != 'v1' || ticks > 2000) t.cancel();
  });
}
