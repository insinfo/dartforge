// A classe do receptor lida em linha e o cache do ponto de chamada
// (`llvm/mod.rs`, `df.classe`/`df.seletor`), o `==` de `String` numa
// chamada (`DartForge_string_igual_a`), a string canônica de um caractere
// do `s[i]`, a limpeza da exceção só quando pendente, e o `Ref` atribuído
// capturado por funções locais diretas, que mora no slot do quadro de
// raízes (`funcoes_diretas.rs`). A saída tem de ser a da VM.
import 'dart:async';

class Ponto {
  final int x, y;
  Ponto(this.x, this.y);
  @override
  bool operator ==(Object o) => o is Ponto && o.x == x && o.y == y;
  @override
  int get hashCode => x * 31 + y;
  @override
  String toString() => 'Ponto($x, $y)';
}

class Caixa<T> {
  T valor;
  Caixa(this.valor);
  String descrever() => 'Caixa<$T>($valor)';
}

class Sub extends Caixa<String> {
  Sub(super.valor);
  @override
  String descrever() => 'Sub(${valor.length})';
}

abstract class Forma {
  double area();
}

class Quadrado extends Forma {
  final double l;
  Quadrado(this.l);
  @override
  double area() => l * l;
}

class Circulo extends Forma {
  final double r;
  Circulo(this.r);
  @override
  double area() => 3 * r * r;
}

class Retangulo extends Forma {
  final double a, b;
  Retangulo(this.a, this.b);
  @override
  double area() => a * b;
}

// Um ponto de chamada dinâmico que vê receptores de muitas classes: o
// cache muda de classe a cada volta.
String descrever(dynamic x) {
  final t = x.toString();
  final h = x.hashCode is int;
  return '$t:$h:${x.runtimeType}';
}

// O `Ref` atribuído, capturado só por funções diretas: a função grava por
// endereço no slot do quadro de quem declara; a coleta tem de vê-lo.
List<String> juntarComDiretas(List<String> partes) {
  String atual = '';
  List<String> saida = [];
  Object? ultimo;
  void anexar(String p) {
    // Aloca bastante para provocar coletas com `atual` só no slot.
    var lixo = <List<int>>[];
    for (var i = 0; i < 50; i++) {
      lixo.add(List<int>.filled(20, i));
    }
    atual = atual.isEmpty ? p : '$atual-$p';
    ultimo = lixo.last;
  }

  void fechar() {
    saida.add(atual);
    atual = '';
  }

  void talvez(String p) {
    if (p == '|') {
      fechar();
    } else {
      anexar(p);
    }
  }

  for (final p in partes) {
    talvez(p);
  }
  fechar();
  saida.add('ultimo=${(ultimo as List<int>).length}');
  return saida;
}

// Recursão direta com um `Ref` atribuído de fora.
String arvore(int n) {
  StringBuffer? sb = StringBuffer();
  String rotulo = 'r';
  void visitar(int k) {
    if (k == 0) {
      sb!.write('.');
      return;
    }
    rotulo = '$rotulo$k';
    sb!.write('(');
    visitar(k - 1);
    visitar(k - 1);
    sb!.write(')');
  }

  visitar(n);
  final r = '${sb.toString()} $rotulo';
  sb = null;
  return r;
}

int comFinally(int n) {
  try {
    if (n > 0) throw StateError('n=$n');
    return 1;
  } finally {
    if (n > 1) return -n;
  }
}

String depoisDoCatch() {
  try {
    throw FormatException('x');
  } catch (e, s) {
    final r = comFinally(0);
    return '${e.runtimeType} ${s is StackTrace} $r';
  }
}

Future<int> assincrono(int n) async {
  var soma = 0;
  for (var i = 0; i < n; i++) {
    soma += await Future.value(i);
  }
  return soma;
}

void main() async {
  // `==` de String: literais, construídas, `null`, outro tipo, dois bytes.
  final a = 'hello';
  final b = ['hel', 'lo'].join();
  final Object? nada = null;
  final Object numero = 5;
  final dois = 'café €';
  final dois2 = 'café ' + '€';
  print([a == b, a != b, identical(a, b), a == 'hello', b == 'hellO']);
  print([a == nada, a == numero, (nada as String?) == null, dois == dois2, dois == a]);
  print([a == (Object() as dynamic).toString(), '' == '', 'x' == 'x'.substring(0)]);
  String? talvez = DateTime.now().year > 3000 ? 'nunca' : null;
  print([talvez == null, talvez == 'nunca', null == talvez]);

  // `s[i]`: a string de um caractere (Latin-1 canônica, como a VM).
  final s = 'abé€';
  print([s[0], s[2], s[3], identical(s[0], 'a'), identical(s[1], s[1]), s[0] == 'a']);
  print([for (var i = 0; i < s.length; i++) s[i].codeUnitAt(0)]);
  try {
    print(s[4]);
  } on RangeError catch (e) {
    print('RangeError ${e.start} ${e.end} ${e.invalidValue}');
  }
  var contagem = <String, int>{};
  for (final c in 'mississippi éé'.split('')) {
    contagem[c] = (contagem[c] ?? 0) + 1;
  }
  print(contagem);
  var cs = 'abcabc';
  var iguais = 0;
  for (var i = 0; i < cs.length; i++) {
    if (cs[i] == 'a' || cs[i] == 'c') iguais++;
  }
  print(iguais);

  // Despacho: o mesmo ponto com classes diferentes (objeto do espaço,
  // String, Smi, `_Mint`, double, lista, mapa, null, closure, record).
  final valores = <dynamic>[
    Ponto(1, 2), 'texto', 42, 1 << 62, 2.5, [1, 2], {'k': 1}, null, true,
    Caixa<int>(3), Sub('abc'), (1, 'b'), Quadrado(2), -7,
  ];
  for (var volta = 0; volta < 3; volta++) {
    for (final v in valores) {
      print(descrever(v));
    }
  }
  final formas = <Forma>[Quadrado(2), Circulo(1), Retangulo(2, 3), Quadrado(3)];
  var total = 0.0;
  for (var i = 0; i < 40; i++) {
    total += formas[i % 4].area();
  }
  print(total);
  final caixas = <Caixa>[Caixa<int>(1), Sub('xy'), Caixa<String>('z'), Caixa<double>(1.5)];
  for (final c in caixas) {
    print(c.descrever());
  }
  dynamic d = Ponto(1, 2);
  print([d == Ponto(1, 2), d == 3, d == null, identical(d, d), Ponto(1, 2) == Ponto(2, 1)]);
  final o1 = Object(), o2 = Object();
  print([o1 == o2, o1 == o1, identical(o1, o2), identical(1 << 62, 1 << 62), identical(2.5, 2.5)]);
  print(<Object>{Ponto(1, 2), Ponto(1, 2), 'a', 'a', 1, 1.0}.length);

  // `is` com o Smi e o null em linha.
  final testes = <Object?>[0, -1, 1 << 62, 1.0, null, 'x', Ponto(0, 0)];
  print([for (final t in testes) '${t is int}${t is num}${t is Object}${t is Null}${t is String}']);

  // Funções diretas com `Ref` atribuído.
  print(juntarComDiretas(['a', 'b', '|', 'c', '|', 'd', 'e', 'f']));
  print(arvore(3));

  // Exceções: o `return` do `finally` descarta a pendente; o catch segue.
  print([comFinally(0), comFinally(2)]);
  try {
    comFinally(1);
  } on StateError catch (e) {
    print('pegou ${e.message}');
  }
  print(depoisDoCatch());

  // Genéricos criados pelo runtime (`rti_definir`).
  final f = Future<int>.value(3);
  final lista = <int>[1, 2, 3];
  final mapa = <String, List<int>>{'a': lista};
  final completer = Completer<String>();
  print([f.runtimeType, lista.runtimeType, mapa.runtimeType, completer.future.runtimeType]);
  completer.complete('ok');
  print([await f, await completer.future, await assincrono(5)]);
  print(Caixa<List<String>>(['q']).descrever());
}
