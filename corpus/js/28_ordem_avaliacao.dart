// ordem de avaliação: argumentos, operandos, índices, cascatas, interpolação, condições, literais.
int v(String nome, int valor) {
  print('avaliou $nome');
  return valor;
}

String s(String nome) {
  print('avaliou $nome');
  return nome;
}

int soma3(int a, int b, int c) => a + b + c;

int nomeados({required int x, required int y}) => x * y;

class Caixa {
  final List<String> log = [];
  void add(String x) {
    print('add $x');
    log.add(x);
  }
}

void main() {
  print('--- argumentos posicionais ---');
  print(soma3(v('a', 1), v('b', 2), v('c', 3)));

  print('--- argumentos nomeados fora de ordem ---');
  print(nomeados(y: v('y', 2), x: v('x', 3)));

  print('--- operandos ---');
  print(v('esq', 10) - v('dir', 4));
  print(v('p', 2) * v('q', 3) + v('r', 4));
  print(v('m', 2) + v('n', 3) * v('o', 4));

  print('--- índices ---');
  final lista = [0, 0, 0];
  lista[v('índice', 1)] = v('valor', 7);
  print(lista);
  final mapa = <String, int>{};
  mapa[s('chave')] = v('valor2', 9);
  print(mapa);
  print(lista[v('i1', 0)] + lista[v('i2', 1)]);

  print('--- receptor antes dos argumentos ---');
  final listas = [
    [1, 2],
    [3, 4]
  ];
  print(listas[v('externo', 1)][v('interno', 0)]);

  print('--- cascata ---');
  final caixa = Caixa()
    ..add(s('um'))
    ..add(s('dois'))
    ..add(s('três'));
  print(caixa.log);

  print('--- interpolação ---');
  print('${s('x')}-${s('y')}-${v('z', 1)}');

  print('--- condições ---');
  if (v('c1', 1) < v('c2', 2)) print('menor');
  print(v('t', 1) > 0 ? s('ramoV') : s('ramoF'));

  print('--- literais de lista, set e mapa ---');
  final l = [v('l1', 1), v('l2', 2), v('l3', 3)];
  print(l);
  final st = {s('s1'), s('s2')};
  print(st);
  final m = {s('k1'): v('v1', 1), s('k2'): v('v2', 2)};
  print(m);

  print('--- record ---');
  final rec = (v('r1', 1), nome: s('r2'), v('r3', 3));
  print(rec);

  print('--- atribuição composta ---');
  var acc = 1;
  acc += v('inc', 5) * v('mult', 2);
  print(acc);
  final arr = [1, 2, 3];
  arr[v('ix', 2)] += v('delta', 10);
  print(arr);

  print('--- chamada com spread ---');
  print([...[v('sp1', 1)], v('sp2', 2), ...{v('sp3', 3)}]);

  print('--- inicializadores de variáveis locais ---');
  final a = v('va', 1), b = v('vb', 2);
  print(a + b);

  print('--- operadores de igualdade ---');
  print(v('e1', 1) == v('e2', 1));
  print(s('str1') == s('str2'));
  print('fim');
}
