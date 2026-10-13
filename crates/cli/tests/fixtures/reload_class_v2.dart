import 'dart:async';

class Alfa {
  String saudacao() => 'alfa';
}

// J03: v2 inserts a class before this one (in name order); the live object
// still belongs to this class, with its state and the getter's new body.
class Zeta {
  int valor = 0;
  void subir() => valor += 1;
  String get nome => 'zz';
}

final z = Zeta();
var ticks = 0;

String rotulo() => 'v2';

void main() {
  print('main');
  Timer.periodic(Duration(milliseconds: 40), (t) {
    ticks += 1;
    z.subir();
    print('${rotulo()} ${z.valor} ${z.nome} ${Alfa().saudacao()} ${z is Zeta}');
    if (rotulo() != 'v1' || ticks > 2000) t.cancel();
  });
}
