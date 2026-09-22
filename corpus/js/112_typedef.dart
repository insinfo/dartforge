// typedef: sintaxe antiga e nova de função, genérico, tipos não-função (List, Map, record), uso em parâmetros/campos/is, record nomeado, typedef de typedef.
typedef int Antigo(int x);
typedef Novo = int Function(int);
typedef Binario<T> = T Function(T, T);
typedef Callback = void Function(String msg);
typedef Predicado<T> = bool Function(T);
typedef Nomeado = int Function({required int a, int b});
typedef Opcional = String Function(String, [int]);
typedef Pair<T> = (T, T);
typedef Registro = ({String nome, int idade});
typedef IntList = List<int>;
typedef Dicionario<V> = Map<String, V>;
typedef Matriz = List<List<int>>;
typedef Talvez<T> = T?;
typedef Aninhado = List<Pair<int>>;
typedef Fabrica<T> = T Function();
typedef Curried = int Function(int) Function(int);

int dobra(int x) => x * 2;
int soma(int a, int b) => a + b;
String concat(String a, String b) => a + b;

int aplica(Novo f, int v) => f(v);
T reduz<T>(List<T> xs, Binario<T> f) => xs.reduce(f);
Pair<T> troca<T>(Pair<T> p) => (p.$2, p.$1);
int idade(Registro r) => r.idade;
int somaLista(IntList xs) => xs.fold(0, (a, b) => a + b);

class Botao {
  final Callback aoClicar;
  final List<Predicado<int>> filtros = [];
  Botao(this.aoClicar);
  void clica(String s) => aoClicar('clicou $s');
}

Curried somador = (a) => (b) => a + b;

void main() {
  Antigo a = dobra;
  Novo n = dobra;
  print(a(3));
  print(n(4));
  print(aplica(dobra, 5));
  print(aplica((x) => x - 1, 5));
  print(reduz<int>([1, 2, 3], soma));
  print(reduz<String>(['a', 'b', 'c'], concat));
  print(reduz<int>([4, 5], (x, y) => x * y));
  print('--');
  print(dobra is Antigo);
  print(dobra is Novo);
  print(dobra is Binario<int>);
  print(soma is Binario<int>);
  print(soma is Binario<num>);
  print(concat is Binario<String>);
  print(concat is Binario<int>);
  print('--');
  final Nomeado nom = ({required int a, int b = 10}) => a + b;
  print(nom(a: 1));
  print(nom(a: 1, b: 2));
  final Opcional opt = (s, [n = 2]) => s * n;
  print(opt('ab'));
  print(opt('ab', 3));
  print('--');
  Pair<int> p = (1, 2);
  print(p);
  print(troca(p));
  print(troca(('x', 'y')));
  print(p is Pair<int>);
  print(p is Pair<String>);
  print((1, 2, 3) is Pair<int>);
  final Registro r = (nome: 'Ana', idade: 30);
  print(r);
  print(idade(r));
  print(idade((idade: 7, nome: 'Bia')));
  print(r is Registro);
  print((nome: 'x', idade: 1, extra: true) is Registro);
  print('--');
  IntList xs = [1, 2, 3];
  print(somaLista(xs));
  print(xs is IntList);
  print(['a'] is IntList);
  Dicionario<int> d = {'a': 1};
  d['b'] = 2;
  print(d);
  print(d is Dicionario<int>);
  print(d is Dicionario<String>);
  Matriz m = [[1, 2], [3, 4]];
  print(m[1][0]);
  Talvez<int> t = null;
  print(t);
  t = 5;
  print(t);
  Aninhado an = [(1, 2), (3, 4)];
  print(an.map((p) => p.$1 + p.$2).toList());
  print('--');
  final b = Botao((msg) => print('callback: $msg'));
  b.clica('ok');
  b.filtros.add((x) => x > 2);
  b.filtros.add((x) => x.isEven);
  print([1, 2, 3, 4].where((x) => b.filtros.every((f) => f(x))).toList());
  print(somador(3)(4));
  Fabrica<List<int>> fab = () => [1];
  print(fab()..add(2));
  final lista = <Novo>[dobra, (x) => x + 100];
  print(lista.map((f) => f(1)).toList());
  final Map<String, Binario<int>> ops = {'+': soma, '*': (a, b) => a * b};
  print(ops['*']!(6, 7));
  print('fim');
}
