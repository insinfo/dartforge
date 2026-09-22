// funções como valores: typedef de função, mapa de operações, lista de funções, retorno de função, Function, call explícito.
typedef Op = int Function(int, int);
typedef Pred<T> = bool Function(T);
typedef Transf = String Function(String);
typedef Fabrica<T> = T Function();

int soma(int a, int b) => a + b;
int sub(int a, int b) => a - b;
int mul(int a, int b) => a * b;

final Map<String, Op> operacoes = {
  '+': soma,
  '-': sub,
  '*': mul,
  '~/': (a, b) => a ~/ b,
  '%': (a, b) => a % b,
};

Op escolhe(String simbolo) => operacoes[simbolo] ?? (a, b) => 0;

Op curry(int Function(int, int, int) f, int primeiro) => (b, c) => f(primeiro, b, c);

Transf encadeia(List<Transf> ts) => (s) {
      var r = s;
      for (final t in ts) {
        r = t(r);
      }
      return r;
    };

int chamaDinamico(Function f, List<Object?> args) {
  return Function.apply(f, args) as int;
}

void main() {
  print(operacoes['+']!(2, 3));
  print(operacoes['*']!(4, 5));
  for (final e in operacoes.entries) {
    print('7 ${e.key} 3 = ${e.value(7, 3)}');
  }
  print(escolhe('-')(10, 4));
  print(escolhe('?')(10, 4));

  // lista de funções
  final fs = <Op>[soma, sub, mul, (a, b) => a * a + b * b];
  print(fs.map((f) => f(3, 4)).toList());
  var acumulado = 1;
  for (final f in fs) {
    acumulado = f(acumulado, 2);
  }
  print(acumulado);

  // typedef genérico
  final Pred<int> par = (n) => n.isEven;
  final Pred<String> curta = (s) => s.length < 3;
  print([1, 2, 3, 4].where(par).toList());
  print(['a', 'abc', 'bb'].where(curta).toList());

  // retornando função
  final somaCom10 = curry((a, b, c) => a + b + c, 10);
  print(somaCom10(1, 2));
  final pipeline = encadeia([
    (s) => s.trim(),
    (s) => s.toUpperCase(),
    (s) => '[$s]',
  ]);
  print(pipeline('  olá  '));

  // Fabrica typedef
  final Fabrica<List<int>> novaLista = () => [];
  final l1 = novaLista();
  final l2 = novaLista();
  l1.add(1);
  print('$l1 $l2 ${identical(l1, l2)}');

  // call explícito
  print(soma.call(1, 1));
  final Op m = mul;
  print(m.call(6, 7));
  print(operacoes['-']?.call(9, 4));
  Op? nula;
  print(nula?.call(1, 2));

  // Function genérico e Function.apply
  Function qualquer = soma;
  print(qualquer(5, 6));
  print(chamaDinamico(mul, [3, 3]));
  print(chamaDinamico((int a) => a * 100, [7]));
  print(Function.apply(soma, [20, 22]));

  // atribuição a variável de tipo função e reatribuição
  Op atual = soma;
  print(atual(1, 2));
  atual = sub;
  print(atual(1, 2));
  atual = (a, b) => a * 10 + b;
  print(atual(1, 2));

  // funções em records e como valores de mapa aninhado
  final rec = (nome: 'dobro', f: (int x) => x * 2);
  print('${rec.nome}: ${rec.f(21)}');
  final tabela = <String, Map<String, Op>>{
    'arit': {'+': soma},
    'outra': {'x': mul},
  };
  print(tabela['arit']!['+']!(1, 1));
  print(tabela['outra']!['x']!(3, 3));

  // is com tipos função
  print(soma is Op);
  print(soma is Function);
  print(soma is int Function(int, int));
  print(soma is String Function(int, int));
  print(par is Pred<int>);
  print(par is Pred<num>);

  // função que recebe e devolve função identidade
  Op idOp(Op f) => f;
  print(idOp(soma)(2, 2));
  print(identical(idOp(soma), soma));
}
