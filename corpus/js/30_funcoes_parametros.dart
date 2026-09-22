// parâmetros: posicionais, opcionais posicionais e nomeados com default, required, mistos, defaults const.
String pos(int a, String b) => '$a-$b';

String opcPos(int a, [int b = 2, int? c]) => '$a $b $c';

String nomeados({int x = 1, String s = 'padrão', bool? flag}) => 'x=$x s=$s flag=$flag';

String obrigatorio({required int id, String nome = 'anon', required String email}) =>
    '$id $nome $email';

String misto(int a, int b, {int c = 0, int d = 0}) => '${a + b} c=$c d=$d';

String mistoPos(int a, [int b = 10, int c = 100]) => '${a + b + c}';

List<int> comListaConst([List<int> xs = const [1, 2, 3]]) => xs;

Map<String, int> comMapaConst({Map<String, int> m = const {'k': 1}}) => m;

int aplica(int Function(int, int) op, int a, int b) => op(a, b);

int aplicaNomeada({required int Function(int) f, int v = 5}) => f(v);

String comDefaultExpr([int n = 2 + 3, String s = 'a' 'b']) => '$n $s';

void main() {
  print(pos(1, 'x'));
  print(opcPos(1));
  print(opcPos(1, 5));
  print(opcPos(1, 5, 9));
  print(nomeados());
  print(nomeados(x: 7));
  print(nomeados(s: 'oi', x: 3));
  print(nomeados(flag: true, s: 'z'));
  print(obrigatorio(id: 1, email: 'a@b'));
  print(obrigatorio(email: 'c@d', nome: 'Zé', id: 2));
  print(misto(1, 2));
  print(misto(1, 2, d: 4));
  print(misto(1, 2, d: 4, c: 3));
  print(mistoPos(1));
  print(mistoPos(1, 2));
  print(mistoPos(1, 2, 3));

  // defaults const: a mesma instância, imutável
  final l1 = comListaConst();
  final l2 = comListaConst();
  print(identical(l1, l2));
  print(l1);
  print(comListaConst([9]));
  try {
    l1.add(4);
  } catch (e) {
    print('lista default é imutável: ${e is UnsupportedError}');
  }
  print(comMapaConst());
  print(comMapaConst(m: {'z': 26}));

  // parâmetro de tipo função
  print(aplica((a, b) => a * b, 3, 4));
  print(aplica((a, b) => a - b, 3, 4));
  int somar(int a, int b) => a + b;
  print(aplica(somar, 3, 4));
  print(aplicaNomeada(f: (v) => v * v));
  print(aplicaNomeada(v: 2, f: (v) => -v));

  // default com expressão constante
  print(comDefaultExpr());
  print(comDefaultExpr(1));
  print(comDefaultExpr(1, 'c'));

  // argumento nomeado com valor null explícito mantém null (não usa default)
  String nulavel({String? s = 'd'}) => 's=$s';
  print(nulavel());
  print(nulavel(s: null));
  print(nulavel(s: 'v'));

  // nomeados em qualquer posição entre posicionais
  String entre(int a, {int m = 0}) => '$a $m';
  print(entre(1, m: 2));

  // função com muitos parâmetros
  String muitos(int a, int b, int c, int d, int e, int f) => '$a$b$c$d$e$f';
  print(muitos(1, 2, 3, 4, 5, 6));

  // função sem parâmetros e com parâmetro ignorado
  int semParam() => 42;
  int ignora(int _, int b) => b;
  print(semParam());
  print(ignora(1, 2));
}
