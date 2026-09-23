// Encaminhadores de `noSuchMethod`: o CFE os sintetiza com a assinatura
// completa do membro abstrato, então a `Invocation` recebe todos os
// posicionais (com os defaults), todos os nomeados (com os defaults, na ordem
// da declaração) e os argumentos de tipo. Cada forma de parâmetro aparece
// aqui: nomeados obrigatórios e opcionais, opcionais posicionais, genéricos,
// getters e setters, interface genérica e chamada dinâmica.

abstract class Pintor {
  static const padrao = 'fosco';

  String pinta(int x, {bool brilho = false, required int cor});
  void opcionais(int a, [int b = 3, String? c]);
  String ordem({required String z, int a = 1, String acabamento = padrao});
  T generico<T, U>(T x, U y);
  R limitado<R extends num>(R x, {List<R>? extras});
  int get valor;
  set valor(int v);
  String get somenteLeitura;
  set somenteEscrita(String n);
}

abstract class Caixa<E> {
  void guarda(E x, {int z = 1, required String a, List<int> l = const [1, 2]});
  E get conteudo;
  R converte<R extends num>(E x);
}

mixin Contador {
  int conta(int a, [int passo = 1]);
}

String descreve(Invocation i) {
  final tipo = i.isGetter
      ? 'get'
      : i.isSetter
          ? 'set'
          : 'metodo';
  final nomeados = i.namedArguments.entries.map((e) => '${e.key}=${e.value}').join(', ');
  return '$tipo ${i.memberName} pos=${i.positionalArguments} '
      'nom={$nomeados} tipos=${i.typeArguments}';
}

class Falso with Contador implements Pintor, Caixa<String> {
  final List<String> log = [];

  @override
  dynamic noSuchMethod(Invocation i) {
    log.add(descreve(i));
    if (i.memberName == #pinta || i.memberName == #ordem) return 'p';
    if (i.memberName == #generico || i.memberName == #limitado) return i.positionalArguments[0];
    if (i.memberName == #valor) return 7;
    if (i.memberName == #somenteLeitura || i.memberName == #conteudo) return 's';
    if (i.memberName == #converte) return 1;
    if (i.memberName == #conta) return (i.positionalArguments[0] as int) + (i.positionalArguments[1] as int);
    return null;
  }
}

void main() {
  final f = Falso();
  final Pintor p = f;
  print(p.pinta(1, cor: 2));
  print(p.pinta(1, brilho: true, cor: 3));
  print(p.pinta(1, cor: 4, brilho: false));
  p.opcionais(1);
  p.opcionais(1, 2);
  p.opcionais(1, 2, 'c');
  print(p.ordem(z: 'zz'));
  print(p.ordem(acabamento: 'brilhante', z: 'y', a: 9));
  print(p.generico<int, String>(4, 'y'));
  final double r = p.generico<double, bool>(4.5, true);
  print(r);
  print(p.limitado<int>(3));
  print(p.limitado<double>(2.5, extras: [1.5]));
  print(p.valor);
  p.valor = 9;
  print(p.somenteLeitura);
  p.somenteEscrita = 'n';

  final Caixa<String> c = f;
  c.guarda('x', a: 'b');
  c.guarda('x', a: 'b', z: 5, l: [7]);
  print(c.conteudo);
  print(c.converte<int>('q'));

  print(f.conta(10));
  print(f.conta(10, 5));

  dynamic d = f;
  d.guarda('y', a: 'c');
  print(d.pinta(2, cor: 8));
  d.naoExiste(1, k: 2);
  print(d.generico<String, int>('g', 1));

  for (final linha in f.log) {
    print(linha);
  }
}
