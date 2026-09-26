// Objetos que escapam e sobrevivem: árvores binárias e lista ligada
// (alocação e coleta de verdade).
import 'comum.dart';

class No {
  final No? esq, dir;
  No(this.esq, this.dir);
  int conta() => 1 + (esq?.conta() ?? 0) + (dir?.conta() ?? 0);
}

No arvore(int d) => d == 0 ? No(null, null) : No(arvore(d - 1), arvore(d - 1));

int arvores(int maxD) {
  final longa = arvore(maxD);
  var total = 0;
  for (var d = 4; d <= maxD; d += 2) {
    final it = 1 << (maxD - d + 4);
    for (var i = 0; i < it; i++) {
      total += arvore(d).conta();
    }
  }
  return total + longa.conta();
}

class Elo {
  final int v;
  Elo? prox;
  Elo(this.v, this.prox);
}

int lista(int n) {
  Elo? cab;
  for (var i = 0; i < n; i++) {
    cab = Elo(i, cab);
  }
  var s = 0;
  for (var e = cab; e != null; e = e.prox) {
    s += e.v;
  }
  return s;
}

void main() {
  medir('arvores', () => arvores(14));
  medir('lista_ligada', () => lista(1000000));
}
