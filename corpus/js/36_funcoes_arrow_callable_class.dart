// funções seta e classes com método call usadas como funções (map, tearoff de call, parâmetros nomeados).
int dobro(int x) => x * 2;
bool vazio(String s) => s.isEmpty;
void nada() => print('nada');
List<int> ateN(int n) => [for (var i = 1; i <= n; i++) i];
String Function(String) prefixador(String p) => (s) => '$p$s';

class Somador {
  final int base;
  Somador(this.base);
  int call(int x) => base + x;
}

class Formatador {
  String call(String s, {String prefixo = '<', String sufixo = '>'}) => '$prefixo$s$sufixo';
}

class Contador {
  int n = 0;
  int call() => ++n;
}

class Multiplicador {
  final int k;
  const Multiplicador(this.k);
  int call(int x) => x * k;
}

class SemCall {
  int valor(int x) => x;
}

void main() {
  print(dobro(4));
  print(vazio(''));
  print(vazio('a'));
  nada();
  print(ateN(5));
  print(prefixador('# ')('título'));

  // arrow com corpo de expressão complexa
  final classifica = (int n) => n < 0 ? 'neg' : n == 0 ? 'zero' : 'pos';
  print([-1, 0, 1].map(classifica).toList());

  // arrow retornando record e lista
  final par = (int a, int b) => (min: a < b ? a : b, max: a < b ? b : a);
  print(par(9, 3));

  // classe com call usada como função
  final soma5 = Somador(5);
  print(soma5(10));
  print(soma5.call(1));
  print([1, 2, 3].map(soma5).toList());
  print([1, 2, 3].map(Somador(100)).toList());

  // call com parâmetros nomeados
  final fmt = Formatador();
  print(fmt('x'));
  print(fmt('y', prefixo: '['));
  print(fmt('z', sufixo: ')', prefixo: '('));

  // tearoff de call
  final int Function(int) f = soma5.call;
  print(f(2));
  final g = soma5;
  int Function(int) h = g;
  print(h(3));
  print(soma5 is int Function(int));
  print(soma5 is Function);
  print(SemCall() is Function);

  // call com estado
  final c = Contador();
  c();
  c();
  print(c());
  print(c.n);
  final tearoff = c.call;
  tearoff();
  print(c.n);

  // callable const em lista e passada a fold
  const triplo = Multiplicador(3);
  print(triplo(7));
  final ops = <int Function(int)>[triplo, Multiplicador(2), dobro, (x) => x + 1];
  print(ops.map((op) => op(10)).toList());
  print(ops.fold<int>(1, (acc, op) => op(acc)));

  // callable passada onde se espera Function e chamada dinamicamente
  Function din = soma5;
  print(din(5));

  // arrow que chama callable
  final aplica = (int Function(int) fn, int v) => fn(v);
  print(aplica(soma5, 1));
  print(aplica(triplo, 1));
  print(aplica(dobro, 1));

  // funções seta aninhadas
  final add = (int a) => (int b) => (int c) => a + b + c;
  print(add(1)(2)(3));

  // arrow em forEach com efeito
  var total = 0;
  [1, 2, 3].forEach((x) => total += x);
  print(total);

  // arrow em where/sort
  final xs = [5, 3, 8, 1];
  xs.sort((a, b) => a.compareTo(b));
  print(xs);
  print(xs.where((x) => x > 2).map((x) => x * x).toList());
}
