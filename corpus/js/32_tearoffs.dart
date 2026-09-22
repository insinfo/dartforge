// tearoffs: função de topo, método de instância, estático, construtor (new/nomeado), igualdade, passados a map/forEach.
int dobro(int x) => x * 2;

void imprime(Object o) => print('imprime: $o');

class Ponto {
  final int x, y;
  Ponto(this.x, this.y);
  Ponto.origem()
      : x = 0,
        y = 0;
  Ponto.deLista(List<int> l)
      : x = l[0],
        y = l[1];

  static Ponto unitario() => Ponto(1, 1);
  static int somaEstatica(int a, int b) => a + b;

  int soma() => x + y;
  int escala(int k) => (x + y) * k;
  String descreve([String prefixo = 'P']) => '$prefixo($x, $y)';

  @override
  String toString() => 'Ponto($x, $y)';
}

class Fabrica<T> {
  final T valor;
  Fabrica(this.valor);
  @override
  String toString() => 'Fabrica<$valor>';
}

void main() {
  // tearoff de função de topo
  final f = dobro;
  print(f(21));
  print([1, 2, 3].map(dobro).toList());
  [1, 2].forEach(imprime);

  // tearoff de método de instância
  final p = Ponto(3, 4);
  final soma = p.soma;
  print(soma());
  final escala = p.escala;
  print(escala(10));
  final desc = p.descreve;
  print(desc());
  print(desc('Q'));
  print([1, 2, 3].map(p.escala).toList());

  // tearoff estático
  final u = Ponto.unitario;
  print(u());
  final se = Ponto.somaEstatica;
  print(se(2, 3));
  print([10, 20].map((v) => Ponto.somaEstatica(v, 1)).toList());

  // tearoff de construtor
  final novo = Ponto.new;
  print(novo(7, 8));
  final origem = Ponto.origem;
  print(origem());
  final deLista = Ponto.deLista;
  print(deLista([5, 6]));
  print([
    [1, 2],
    [3, 4]
  ].map(Ponto.deLista).toList());

  // construtor genérico como tearoff
  final fab = Fabrica<int>.new;
  print(fab(5));
  print(['a', 'b'].map(Fabrica<String>.new).toList());

  // igualdade de tearoffs
  print(dobro == dobro);
  final f2 = dobro;
  print(f == f2);
  print(p.soma == p.soma);
  print(soma == p.soma);
  final p2 = Ponto(3, 4);
  print(p.soma == p2.soma);
  print(Ponto.unitario == Ponto.unitario);
  print(Ponto.new == Ponto.new);
  print(Ponto.origem == Ponto.origem);
  print(identical(dobro, dobro));

  // tearoff guarda o receptor no momento do tearoff
  var atual = Ponto(1, 1);
  final somaGuardada = atual.soma;
  atual = Ponto(50, 50);
  print(somaGuardada());
  print(atual.soma());

  // chamada via variável tipada e via Function
  int Function(int) g = dobro;
  print(g(4));
  Function h = dobro;
  print(h(5));

  // tearoff de método em lista de funções
  final ops = [p.soma, p2.soma, Ponto(10, 20).soma];
  print(ops.map((o) => o()).toList());

  // tearoff de toString
  final ts = p.toString;
  print(ts());

  // tearoff de operador via closure e método de String
  final up = 'abc'.toUpperCase;
  print(up());
  print(['x', 'e', '.'].map('pre-'.padRight(6, '.').contains).toList());
}
