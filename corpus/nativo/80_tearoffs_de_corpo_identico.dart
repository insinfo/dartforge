// Tear-offs de funções distintas com o MESMO corpo continuam distintos: o
// `==` compara a função (o endereço do código no executável nativo). A
// ligação de produção junta funções idênticas (`/opt:safeicf`,
// `--icf=safe`) só quando ninguém toma o endereço delas; um ICF completo
// faria `soma1 == soma1b` dar `true`. Cobre funções de topo, estáticos,
// métodos de instância (o mesmo receptor), extensões e o uso dos tear-offs
// como chaves de mapa e elementos de conjunto.

int soma1(int x) => x + 1;
int soma1b(int x) => x + 1;
int soma1c(int x) => x + 1;

String nome(Object? o) => 'v:$o';
String nomeb(Object? o) => 'v:$o';

void nada() {}
void nadab() {}

class A {
  static int dobro(int x) => x * 2;
  static int dobrob(int x) => x * 2;
  static int soma1(int x) => x + 1;

  final int k;
  A(this.k);

  int m(int x) => x + k;
  int mb(int x) => x + k;
}

class B {
  final int k;
  B(this.k);

  int m(int x) => x + k;
}

extension on int {
  int mais(int y) => this + y;
  int maisb(int y) => this + y;
}

typedef F = int Function(int);

void comparar(String rotulo, Object a, Object b) {
  print('$rotulo: == ${a == b}, identical ${identical(a, b)}, hash ${a.hashCode == b.hashCode ? "igual" : "diferente"}');
}

void main() {
  // Funções de topo de corpo idêntico.
  comparar('soma1/soma1', soma1, soma1);
  print('soma1/soma1b: ${soma1 == soma1b}');
  print('soma1/soma1c: ${soma1 == soma1c}');
  print('soma1b/soma1c: ${soma1b == soma1c}');
  print('nome/nomeb: ${nome == nomeb}');
  print('nada/nadab: ${nada == nadab}');
  print('identical soma1/soma1b: ${identical(soma1, soma1b)}');

  // Estáticos idênticos entre si e a uma função de topo.
  comparar('dobro/dobro', A.dobro, A.dobro);
  print('dobro/dobrob: ${A.dobro == A.dobrob}');
  print('A.soma1/soma1: ${A.soma1 == soma1}');
  print('A.soma1/soma1b: ${A.soma1 == soma1b}');

  // Métodos de instância: o mesmo receptor, corpos idênticos.
  final a = A(3);
  final a2 = A(3);
  final b = B(3);
  comparar('a.m/a.m', a.m, a.m);
  print('a.m/a.mb: ${a.m == a.mb}');
  print('a.m/a2.m: ${a.m == a2.m}');
  print('a.m/b.m: ${a.m == b.m}');

  // Extensões: o tear-off é uma closure nova a cada vez.
  final F e1 = 5.mais;
  final F e2 = 5.maisb;
  print('ext mais/maisb: ${e1 == e2} ${e1(1)} ${e2(1)}');

  // Chaves de mapa e elementos de conjunto.
  final tabela = <F, String>{soma1: 'soma1', soma1b: 'soma1b', soma1c: 'soma1c', A.soma1: 'A.soma1', A.dobro: 'dobro', A.dobrob: 'dobrob'};
  print('tabela: ${tabela.length} ${tabela[soma1]} ${tabela[soma1b]} ${tabela[soma1c]} ${tabela[A.soma1]} ${tabela[A.dobrob]}');
  final conjunto = <Function>{soma1, soma1b, soma1c, soma1, A.soma1, nome, nomeb, nada, nadab, a.m, a.mb, a.m};
  print('conjunto: ${conjunto.length}');

  // Uma lista de tear-offs, deduplicada pelo `==`.
  final fs = <F>[soma1, soma1b, soma1c, A.soma1, A.dobro, A.dobrob, soma1b];
  final unicas = <F>[];
  for (final f in fs) {
    if (!unicas.any((u) => u == f)) unicas.add(f);
  }
  print('unicas: ${unicas.length} ${unicas.map((f) => f(10)).join(",")}');
  print('indexOf: ${fs.indexOf(soma1b)} ${fs.indexOf(A.dobrob)} ${fs.lastIndexOf(soma1b)}');

  // Constantes: `const` canoniza o mesmo tear-off, não os idênticos.
  const c1 = soma1;
  const c2 = soma1b;
  print('const: ${identical(c1, soma1)} ${identical(c1, c2)} ${c1 == c2}');
  const lista = [soma1, soma1b, A.dobro, A.dobrob];
  print('const lista: ${lista[0] == lista[1]} ${lista[2] == lista[3]} ${lista[0] == soma1}');

  // As chamadas continuam certas.
  print('chamadas: ${soma1(1)} ${soma1b(2)} ${soma1c(3)} ${A.dobro(4)} ${A.dobrob(5)} ${a.m(1)} ${a.mb(2)} ${nome(1)} ${nomeb(2)}');
}
