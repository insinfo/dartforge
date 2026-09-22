// operador condicional ternário: aninhado, tipos mistos, precedência com ??, em argumentos e interpolação.
String sinal(int n) => n > 0 ? 'positivo' : n < 0 ? 'negativo' : 'zero';

int abs(int n) => n < 0 ? -n : n;

void mostra(Object o) => print('mostra: $o');

void main() {
  print(sinal(5));
  print(sinal(-5));
  print(sinal(0));
  print(abs(-7));
  print(abs(7));

  // ternário aninhado no ramo verdadeiro (com parênteses para clareza)
  final n = 15;
  final faixa = n < 10 ? 'baixo' : (n < 20 ? 'médio' : 'alto');
  print(faixa);

  // tipos diferentes: num e String em interpolação
  final misto = n.isEven ? 3.5 : 'ímpar';
  print('misto=$misto');
  final misto2 = n.isOdd ? 3.5 : 'par';
  print('misto2=$misto2');

  // tipo inferido: num
  final numero = n > 10 ? 2.5 : 7;
  print(numero);
  print(numero is num);

  // precedência: ?? liga mais forte que ?:
  String? nulo;
  String? preenchido = 'ok';
  print(nulo ?? 'padrão');
  final r1 = nulo == null ? 'era nulo' : nulo;
  print(r1);
  final r2 = (preenchido ?? nulo) != null ? 'a' : 'b';
  print(r2);
  bool? flag;
  print(flag ?? true ? 'flag nulo vira true' : 'flag falso');
  flag = false;
  print(flag ?? true ? 'flag nulo vira true' : 'flag falso');
  final r3 = (nulo ?? 'x') == 'x' ? 'sim' : 'não';
  print(r3);
  final r4 = nulo != null ? nulo : preenchido ?? 'fallback';
  print(r4);

  // ternário em argumentos
  mostra(n > 10 ? 'grande' : 'pequeno');
  mostra(n > 100 ? 1 : 2);
  print([1, 2, 3].map((x) => x.isEven ? 'p' : 'i').join());

  // ternário em interpolação
  print('n é ${n.isEven ? 'par' : 'ímpar'}');
  print('${n > 0 ? '+' : '-'}$n');
  print('${n > 0 ? n > 10 ? 'muito' : 'pouco' : 'nada'}');

  // ternário com chamadas e efeito só no ramo escolhido
  int lado(String s) {
    print('avaliou $s');
    return s.length;
  }

  final v = n > 0 ? lado('esq') : lado('dir');
  print(v);

  // ternário em atribuição composta e em índice
  final lista = ['a', 'b', 'c'];
  print(lista[n > 0 ? 2 : 0]);
  var acc = 0;
  for (var i = 0; i < 5; i++) {
    acc += i.isEven ? i : -i;
  }
  print(acc);

  // ternário com bool e lógica
  final b = n > 0 ? true : false;
  print(b && n > 10 ? 'ambos' : 'não ambos');

  // ternário retornando funções
  final op = n > 0 ? (int a, int b) => a + b : (int a, int b) => a - b;
  print(op(10, 3));

  // encadeado longo
  final nota = 85;
  final conceito = nota >= 90
      ? 'A'
      : nota >= 80
          ? 'B'
          : nota >= 70
              ? 'C'
              : 'D';
  print(conceito);
}
