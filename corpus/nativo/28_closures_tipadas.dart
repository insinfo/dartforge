// Closures com corpo de ABI tipada (`lower/closures.rs`): a chamada com o
// tipo estático certo vai direto ao corpo (`int`, `double`, `bool` sem
// caixa); com outra forma — `dynamic`, `Function.apply`, subtipo com outra
// representação, classe chamável — segue pela entrada uniforme, com o mesmo
// resultado e os mesmos erros.
class Somador {
  final int k;
  Somador(this.k);
  int call(int x) => x + k;
}

int aplicar(int Function(int) f, int x) => f(x);
num aplicarNum(num Function(num) f, num x) => f(x);
Object? aplicarDin(dynamic f, Object? x) => f(x);

void main() {
  int Function(int) somador(int k) => (x) => x + k;
  final fs = [for (var k = 0; k < 8; k++) somador(k)];
  var s = 0;
  for (var i = 0; i < 1000; i++) {
    s = fs[i & 7](s) & 0xFFFFFF;
  }
  print('somador $s');

  // double, bool e retorno Ref (String) e void.
  double Function(double, double) media = (a, b) => (a + b) / 2;
  bool Function(int) par = (x) => x.isEven;
  String Function(int, bool) texto = (n, b) => '$n:$b';
  var chamadas = 0;
  void Function() conta = () {
    chamadas++;
  };
  conta();
  conta();
  print('${media(1.5, 2.25)} ${par(3)} ${par(4)} ${texto(7, true)} $chamadas');

  // Captura mutável compartilhada entre duas closures.
  var total = 0;
  int Function(int) acumula = (x) => total += x;
  int Function() le = () => total;
  acumula(5);
  acumula(10);
  print('captura ${le()} $total');

  // Chamada dinâmica, Function.apply e tipo errado pela entrada uniforme.
  print('dinamico ${aplicarDin(somador(3), 4)}');
  print('apply ${Function.apply(somador(3), [9])}');
  try {
    aplicarDin(somador(3), 'x');
  } on TypeError {
    print('TypeError no argumento');
  }

  // Subtipo com outra representação: `(num x) => x * 2` visto como
  // `int Function(int)` (num é mais largo que int no parâmetro) e o inverso
  // não permitido; o resultado continua certo.
  num Function(num) dobraNum = (x) => x * 2;
  print('num ${aplicarNum(dobraNum, 3)} ${aplicarNum(dobraNum, 1.5)}');
  final int Function(int) comoInt = (num x) => (x * 2).toInt();
  print('subtipo ${aplicar(comoInt, 21)}');

  // Classe chamável e tear-off pelo mesmo tipo de função.
  print('chamavel ${aplicar(Somador(10).call, 5)} ${aplicar(somador(1), 1)}');

  // Exceção de dentro da closure tipada.
  int Function(int) divide = (x) => 100 ~/ x;
  try {
    print(divide(0));
  } on UnsupportedError catch (e) {
    print('excecao ${e.runtimeType}');
  } on IntegerDivisionByZeroException {
    print('excecao divisao');
  }
  print('divide ${divide(7)}');

  // Recursão por closure e retorno anulável (sempre `Ref`).
  late int Function(int) fib;
  fib = (n) => n < 2 ? n : fib(n - 1) + fib(n - 2);
  int? Function(int) talvez = (x) => x > 0 ? x : null;
  print('fib ${fib(20)} ${talvez(3)} ${talvez(-1)}');

  // Opcionais e nomeados continuam pela entrada uniforme.
  int Function(int, [int]) opc = (a, [b = 10]) => a + b;
  int Function({required int a, int b}) nom = ({required a, b = 1}) => a * b;
  print('opcionais ${opc(1)} ${opc(1, 2)} ${nom(a: 3)} ${nom(a: 3, b: 4)}');

  // int grande (fora do Smi) e double especial pela ABI tipada.
  int Function(int) ident = (x) => x;
  double Function(double) negd = (x) => -x;
  print('extremos ${ident(-9223372036854775808)} ${ident(1 << 62)} ${negd(0.0)} ${negd(double.nan)}');
}
