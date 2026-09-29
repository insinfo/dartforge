// Closures de ambiente direto (`lower/closures.rs`): com um valor só no
// ambiente — o `this` sem capturas, ou uma captura `Ref` (ou a célula
// dela) —, a closure guarda o valor no lugar do ambiente. Cobre `this`,
// captura de objeto, de `String`, de `int?`, de null, de célula (atribuída
// antes e depois), capturas escalares (ficam no ambiente), aninhadas,
// função local, igualdade e hash, coleta com muitas closures vivas,
// `Isolate.run`, `async` e gerador (ficam no ambiente). A saída tem de ser
// a da VM.
import 'dart:isolate';

class Contador {
  int n = 0;
  String nome;
  Contador(this.nome);
  void Function() incrementador() => () => n++;
  String Function() descritor() => () => '$nome:$n';
  int Function(int) somador() => (x) => n + x;
  List<String Function()> varios() => [for (var i = 0; i < 3; i++) () => '$nome#$i'];
}

List<void Function()> acoes = [];

String Function() capturaTexto(String s) => () => s.toUpperCase();
int? Function() capturaIntAnulavel(int? v) => () => v == null ? null : v + 1;
Object? Function() capturaNulo() {
  Object? x;
  return () => x;
}

int Function() capturaCelula() {
  var total = 10;
  final f = () => total += 5;
  total = 100;
  return f;
}

int Function() capturaEscalar(int a) => () => a * 2;
double Function() capturaDouble(double d) => () => d / 2;
bool Function() capturaBool(bool b) => () => !b;

String Function() aninhada(List<int> lista) {
  return () {
    final interna = () => lista.length;
    int soma() {
      var s = 0;
      for (final v in lista) s += v;
      return s;
    }
    return '${interna()} ${soma()}';
  };
}

void main() async {
  final c = Contador('c');
  final inc = c.incrementador();
  inc();
  inc();
  print(c.descritor()());
  print(c.somador()(5));
  print(c.varios().map((f) => f()).join(','));

  print(capturaTexto('abc')());
  print(capturaIntAnulavel(41)());
  print(capturaIntAnulavel(null)());
  print(capturaNulo()());
  final cel = capturaCelula();
  print('${cel()} ${cel()}');
  print(capturaEscalar(21)());
  print(capturaDouble(3.0)());
  print(capturaBool(false)());
  print(aninhada([1, 2, 3, 4])());

  // Igualdade e hash: duas avaliações da mesma expressão sobre o mesmo
  // objeto não são iguais; a mesma closure é igual a si mesma.
  final o = Object();
  Object Function() mk() => () => o;
  final a = mk(), b = mk();
  print('${a == b} ${a == a} ${identical(a, a)} ${a.hashCode == a.hashCode}');
  final s1 = capturaTexto('x'), s2 = capturaTexto('x');
  print('${s1 == s2} ${s1() == s2()}');
  final t1 = c.descritor, t2 = c.descritor; // tear-off: iguais
  print('${t1 == t2} ${t1.hashCode == t2.hashCode}');

  // Muitas closures vivas com a coleta no meio.
  final fs = <String Function()>[];
  for (var i = 0; i < 20000; i++) {
    final texto = 'item$i';
    fs.add(() => texto);
    if (i % 3 == 0) fs.add(capturaTexto('t$i'));
    if (i % 5 == 0) acoes.add(Contador('k$i').incrementador());
  }
  for (final f in acoes) {
    f();
  }
  var comprimento = 0;
  for (final f in fs) {
    comprimento += f().length;
  }
  print('${fs.length} $comprimento ${fs[12345]()} ${fs.last()}');

  // Isolate.run com uma captura só.
  final lista = [5, 6, 7];
  print(await Isolate.run(() => lista.reduce((x, y) => x + y)));

  // `async` e gerador com uma captura: ficam no ambiente.
  final nome = 'assinc';
  Future<String> Function() fa = () async => '$nome!';
  print(await fa());
  Iterable<String> Function() gen = () sync* {
    yield nome;
    yield nome.length.toString();
  };
  print(gen().toList());

  // Closure sobre o `this` passada adiante e chamada muito depois.
  final contadores = [for (var i = 0; i < 4; i++) Contador('x$i')];
  final incs = [for (final k in contadores) k.incrementador()];
  for (var r = 0; r < 3; r++) {
    for (final f in incs) {
      f();
    }
  }
  print(contadores.map((k) => k.descritor()()).join(' '));
}
