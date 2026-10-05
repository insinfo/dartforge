// D6 (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3): valores brutos vivos
// com referências através de chamadas que alocam. Um `double` e um `int` par
// desencaixotados nunca podem entrar no mapa como raiz; a sabotagem
// `bruto_no_mapa` põe um par nele e o coletor tem de recusá-lo.

class P {
  final double x;
  final int y;
  P(this.x, this.y);
}

String rotulo(int i) => 'r$i';

void main() {
  var d = 0.0;
  var n = 0;
  var tam = 0;
  final ps = <P>[];
  for (var i = 0; i < 1000; i++) {
    final dd = i * 0.5;
    final ii = i * 2;
    final r = rotulo(i);
    ps.add(P(dd, ii));
    d += dd;
    n += ii;
    tam += r.length;
  }
  var s = 0.0;
  for (final p in ps) {
    s += p.x + p.y;
  }
  print('$d $n $tam $s');
}
