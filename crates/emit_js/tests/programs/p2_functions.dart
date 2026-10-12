int soma(int a, int b) => a + b;
int fat(int n) => n <= 1 ? 1 : n * fat(n - 1);
String opc(String a, [String b = 'b', String? c]) => '$a$b$c';
String nom(String a, {int x = 1, required String y, bool? z}) => '$a$x$y$z';
T primeiro<T>(List<T> l) => l[0];
T? talvez<T extends Object>(T? v) => v;
void aplica(int Function(int) f, int v) => print(f(v));
int Function(int) faz(int k) => (int x) => x * k;
int contador = 0;
final String nome = 'n';
late int tarde;
const double pi2 = 6.28;
int get dobro => contador * 2;
set dobro(int v) { contador = v ~/ 2; }
void vazio() {}
void main() {
  print(soma(1, 2));
  print(fat(5));
  print(opc('a'));
  print(opc('a', 'x'));
  print(opc('a', 'x', 'y'));
  print(nom('a', y: 'y'));
  print(nom('a', x: 5, y: 'y', z: true));
  print(primeiro([1, 2]));
  print(primeiro(<String>['s']));
  print(talvez<int>(null));
  aplica((x) => x + 1, 1);
  aplica(faz(3), 2);
  var f = soma;
  print(f(2, 3));
  var g = fat;
  print(g(3));
  int local(int x) { return x * 10; }
  print(local(4));
  var lst = [3, 1, 2];
  lst.sort((a, b) => a - b);
  print(lst);
  print(lst.map((e) => e * 2).toList());
  contador = 5;
  print(dobro);
  dobro = 20;
  print(contador);
  tarde = 3;
  print(tarde);
  print(pi2);
  print(nome);
  var h = primeiro<int>;
  print(h([9]));
  Function dyn = soma;
  print(dyn(1, 1));
  var cl = () { return 42; };
  print(cl());
  int acc = 0;
  void inc() { acc++; }
  inc(); inc();
  print(acc);
  var fns = <void Function()>[];
  for (var i = 0; i < 3; i++) { fns.add(() => print(i)); }
  for (var fn in fns) fn();
  vazio();
  print(soma.runtimeType);
  print([1, 2, 3].fold<int>(0, (a, b) => a + b));
  print(Object());
  var rec = fat;
  print(rec == fat);
  int Function(int, int) op = (a, b) => a * b;
  print(op(6, 7));
  var named = ({int a = 1, int b = 2}) => a + b;
  print(named(b: 5));
  var opt = ([int a = 1]) => a;
  print(opt());
  print(opt(7));
}
