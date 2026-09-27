import 'dart:async';

// J03: a v2 insere uma classe antes desta (na ordem dos nomes); o objeto vivo
// continua sendo desta classe, com o estado e o corpo novo do getter.
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
