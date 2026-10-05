// D11 (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3): um laço lê os campos
// de um objeto cuja única referência é o parâmetro, alocando a cada volta;
// só o mapa mantém a base viva.

class Q {
  final int a;
  final int b;
  Q(this.a, this.b);
}

int soma(Q q) {
  var s = 0;
  for (var i = 0; i < 100; i++) {
    s += q.a + q.b;
    final t = 'x$i';
    s += t.length;
  }
  return s;
}

void main() {
  var total = 0;
  for (var k = 0; k < 20; k++) {
    total += soma(Q(k, 1));
  }
  print(total);
}
