import 'dart:async';

class Alfa {
  String saudacao() => 'alfa';
}

// J03: a v2 insere uma classe antes desta (na ordem dos nomes); o objeto vivo
// continua sendo desta classe, com o estado e o corpo novo do getter.
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
