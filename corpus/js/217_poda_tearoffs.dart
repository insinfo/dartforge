// Poda do perfil de produção (docs/JS-PRODUCAO.md §1.7): tearoffs de
// instância, estático, de topo, de construtor (sem nome, nomeado, genérico,
// de fábrica), igualdade de tearoffs, `Function.apply` e classe chamável.
// Se a poda remover o alvo de um tearoff, o caso imprime "caso <nome>: ERRO".

void caso(String nome, Object? Function() f) {
  try {
    print('$nome: ${f()}');
  } catch (e) {
    print('caso $nome: ERRO $e');
  }
}

class Ponto {
  final int x, y;
  Ponto(this.x, this.y);
  Ponto.origem() : this(0, 0);
  factory Ponto.diagonal(int n) => Ponto(n, n);
  int soma() => x + y;
  static Ponto dobro(Ponto p) => Ponto(p.x * 2, p.y * 2);
  @override
  String toString() => 'Ponto($x, $y)';
}

class Caixa<T> {
  final T valor;
  Caixa(this.valor);
  @override
  String toString() => 'Caixa<$valor>';
}

class Somador {
  int call(int a, int b) => a + b;
}

int triplo(int n) => n * 3;

void main() {
  final p = Ponto(1, 2);
  caso('tearoff de instância', () {
    final f = p.soma;
    return f();
  });
  caso('tearoff estático', () => [p].map(Ponto.dobro).toList());
  caso('tearoff de topo', () => [1, 2].map(triplo).toList());
  caso('construtor sem nome', () {
    final f = Ponto.new;
    return f(3, 4);
  });
  caso('construtor nomeado', () {
    final f = Ponto.origem;
    return f();
  });
  caso('fábrica', () => [5].map(Ponto.diagonal).toList());
  caso('construtor genérico', () => ['a'].map(Caixa<String>.new).toList());
  caso('igualdade', () => identical(triplo, triplo) && Ponto.new == Ponto.new);
  caso('Function.apply', () => Function.apply(Ponto.new, [7, 8]));
  caso('classe chamável', () {
    final s = Somador();
    int Function(int, int) f = s.call;
    return '${s(1, 2)} ${f(3, 4)}';
  });
}
