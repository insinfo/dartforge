// D3 (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3): exceção profunda e o
// topo da pilha-sombra. Uma recursão de mil níveis lança no fundo e pega no
// meio; o tratador percorre a lista chamando uma função com quadro que
// aloca, e depois o programa aloca muito. Com `--excecoes=tabelas` e a
// pilha-sombra, a sabotagem `pouso_sem_topo` (o pouso não restaura o topo
// da pilha-sombra) o derruba: o quadro da chamada no tratador encadeia o
// topo morto, e a coleta o acusa.

class No {
  final int v;
  final No? prox;
  No(this.v, this.prox);
}

// Recursiva (fica fora de linha): um quadro próprio e uma alocação com `n`
// vivo. Devolve sempre 1.
int pesa(No n, int k) {
  final t = No(n.v, n);
  if (k > 0) return pesa(t, k - 1);
  return t.prox == null ? 0 : 1;
}

int desce(int n, No? acc) {
  final aqui = No(n, acc);
  if (n == 0) throw StateError('fundo');
  if (n == 500) {
    try {
      return desce(n - 1, aqui);
    } on StateError {
      var c = 0;
      No? p = aqui;
      while (p != null) {
        c += pesa(p, 1);
        p = p.prox;
      }
      return c;
    }
  }
  return desce(n - 1, aqui);
}

void main() {
  var soma = 0;
  for (var k = 0; k < 5; k++) {
    final guarda = <No>[];
    for (var i = 0; i < 100; i++) {
      guarda.add(No(i, null));
    }
    final r = desce(1000, null);
    final muitos = <No>[];
    for (var i = 0; i < 1000; i++) {
      muitos.add(No(i, null));
    }
    var g = 0;
    for (final n in guarda) {
      g += n.v;
    }
    soma += r + muitos.length + g;
  }
  print(soma);
}
