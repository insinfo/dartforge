// closures capturando variáveis de laço: for clássico (nova por iteração), while (compartilhada), for-in, contadores.
void main() {
  // for clássico: cada iteração tem sua própria variável
  final fs = <int Function()>[];
  for (var i = 0; i < 3; i++) {
    fs.add(() => i);
  }
  print(fs.map((f) => f()).toList());

  // for clássico: o incremento age sobre uma cópia nova; modificar dentro da closure não afeta as outras
  final gs = <int Function()>[];
  for (var i = 0; i < 3; i++) {
    gs.add(() => i * 10);
    i += 0;
  }
  print(gs.map((f) => f()).toList());

  // while: a variável é uma só, compartilhada
  final hs = <int Function()>[];
  var j = 0;
  while (j < 3) {
    hs.add(() => j);
    j++;
  }
  print(hs.map((f) => f()).toList());

  // variável declarada dentro do corpo do while é nova a cada iteração
  final ks = <int Function()>[];
  var w = 0;
  while (w < 3) {
    final copia = w;
    ks.add(() => copia);
    w++;
  }
  print(ks.map((f) => f()).toList());

  // for-in: cada iteração tem variável própria
  final ls = <String Function()>[];
  for (final s in ['a', 'b', 'c']) {
    ls.add(() => s.toUpperCase());
  }
  print(ls.map((f) => f()).toList());

  // for-in com var: também própria por iteração
  final ms = <int Function()>[];
  for (var n in [1, 2, 3]) {
    ms.add(() => n * n);
    n = 99;
  }
  print(ms.map((f) => f()).toList());

  // closure modificando a variável capturada do for clássico
  final incs = <void Function()>[];
  final lers = <int Function()>[];
  for (var i = 0; i < 2; i++) {
    incs.add(() => i += 100);
    lers.add(() => i);
  }
  incs[0]();
  incs[0]();
  print(lers.map((f) => f()).toList());

  // contadores independentes criados em laço
  final contadores = <int Function()>[];
  for (var i = 0; i < 3; i++) {
    var c = i * 10;
    contadores.add(() => ++c);
  }
  print(contadores[0]());
  print(contadores[0]());
  print(contadores[1]());
  print(contadores[2]());
  print(contadores[0]());

  // closure modificando variável externa ao laço
  var total = 0;
  final adds = <void Function()>[];
  for (var i = 1; i <= 4; i++) {
    adds.add(() => total += i);
  }
  for (final a in adds) {
    a();
  }
  print('total $total');
  adds[3]();
  print('total $total');

  // do-while: variável compartilhada declarada fora
  final ds = <int Function()>[];
  var d = 0;
  do {
    ds.add(() => d);
    d++;
  } while (d < 3);
  print(ds.map((f) => f()).toList());

  // closures aninhadas em laços aninhados
  final ns = <String Function()>[];
  for (var i = 0; i < 2; i++) {
    for (var k = 0; k < 2; k++) {
      ns.add(() => '$i$k');
    }
  }
  print(ns.map((f) => f()).join(','));

  // closure capturando variável de laço em forEach
  final es = <int Function()>[];
  [5, 6, 7].forEach((x) => es.add(() => x));
  print(es.map((f) => f()).toList());

  // closure criada em laço e chamada depois que o laço altera o estado externo
  var estado = 'inicial';
  final leEstado = <String Function()>[];
  for (var i = 0; i < 2; i++) {
    leEstado.add(() => '$i:$estado');
  }
  estado = 'final';
  print(leEstado.map((f) => f()).toList());
}
