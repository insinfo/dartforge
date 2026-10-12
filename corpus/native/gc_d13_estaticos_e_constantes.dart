// D13 (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3): literais da imagem
// (objetos estáticos) como referências vivas através de chamadas que alocam
// (contrato C2, item 3).

const k1 = 'constante';

String g(int i) => '$k1$i';

void main() {
  var n = 0;
  for (var i = 0; i < 500; i++) {
    final a = 'literal';
    final b = g(i);
    n += a.length + b.length + k1.length;
  }
  print(n);
}
