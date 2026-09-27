import 'dart:async';

// J03: a geração do meio não tem a classe Zeta; o objeto vivo (em `z`, que
// fica na área de globais) continua sendo dela, e a geração seguinte, que a
// traz de volta, tem de dar a ela o mesmo id.
var ticks = 0;

String rotulo() => 'vb';

void main() {
  print('main');
  Timer.periodic(Duration(milliseconds: 40), (t) {
    ticks += 1;
    print('${rotulo()} $ticks');
  });
}
