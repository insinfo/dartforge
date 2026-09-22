// Null safety: promoção de locais, ??, late, required, ! que falha, && / is / early return, campos não promovem.
class Nodo {
  final int valor;
  Nodo? proximo;
  Nodo(this.valor, [this.proximo]);
}

class Config {
  String? nome;
  int? tamanho;
  Config({this.nome, this.tamanho});

  String descricao() {
    final n = nome;
    if (n != null) {
      return 'nome de ${n.length} letras';
    }
    return 'sem nome';
  }

  int tamanhoOuZero() => tamanho ?? 0;
}

int comprimento(String? s) {
  if (s == null) return -1;
  return s.length;
}

String classifica(Object? o) {
  if (o == null) return 'nulo';
  if (o is int && o > 10) return 'int grande $o';
  if (o is int) return 'int ${o + 1}';
  if (o is String && o.isNotEmpty) return 'texto ${o[0]}';
  return 'outro';
}

int somaLista(List<int?> xs) {
  var total = 0;
  for (final x in xs) {
    if (x != null) total += x;
  }
  return total;
}

String requerido({required String nome, int? idade}) =>
    '$nome:${idade ?? '?'}';

int? primeiroPar(List<int> xs) {
  for (final x in xs) {
    if (x.isEven) return x;
  }
  return null;
}

void main() {
  String? s = 'abc';
  if (s != null) {
    print(s.length);
  }
  s = null;
  print(s?.length);
  print(comprimento('xyz'));
  print(comprimento(null));

  int? n = 5;
  if (n != null && n > 3) {
    print(n + 1);
  }
  if (n == null || n < 3) {
    print('nao passa');
  } else {
    print(n * 2);
  }
  print(n != null ? n.isOdd : 'nulo');

  print(classifica(null));
  print(classifica(42));
  print(classifica(3));
  print(classifica('oi'));
  print(classifica(''));
  print(classifica(1.5));

  final c = Config(nome: 'abcd');
  print(c.descricao());
  print(c.tamanhoOuZero());
  print(Config().descricao());
  print(Config(tamanho: 9).tamanhoOuZero());
  final nomeLocal = c.nome;
  print(nomeLocal != null ? nomeLocal.toUpperCase() : '-');
  print(c.nome?.toUpperCase());

  final lista = Nodo(1, Nodo(2, Nodo(3)));
  Nodo? atual = lista;
  final valores = <int>[];
  while (atual != null) {
    valores.add(atual.valor);
    atual = atual.proximo;
  }
  print(valores);
  print(lista.proximo?.proximo?.valor);
  print(lista.proximo?.proximo?.proximo?.valor);

  print(somaLista([1, null, 2, null, 3]));
  print(requerido(nome: 'x'));
  print(requerido(nome: 'y', idade: 7));
  print(primeiroPar([1, 3, 4, 6]));
  print(primeiroPar([1, 3]));
  final pp = primeiroPar([8]);
  if (pp != null) print(pp ~/ 2);

  int? vazio;
  try {
    final int forcado = vazio!;
    print(forcado);
  } catch (e) {
    print('bang falhou: ${e is TypeError}');
  }
  int? cheio = 3;
  print(cheio! + 1);
  print(cheio + 1);

  final mistos = <int?>[1, null, 2];
  print(mistos.whereType<int>().toList());
  print(mistos.where((x) => x != null).map((x) => x! * 10).toList());
  print(mistos.nonNulls.toList());
  final List<int?> nulos = [null, null];
  print(nulos.every((x) => x == null));
  print(mistos.indexOf(null));
  final Map<String, int?> m = {'a': 1, 'b': null};
  print(m['a']);
  print(m['b']);
  print(m['c']);
  print(m.containsKey('b'));
  print(m.values.whereType<int>().length);

  late String tardia;
  var inicializada = false;
  void prepara() {
    tardia = 'pronta';
    inicializada = true;
  }

  prepara();
  print(inicializada);
  print(tardia);
  String? ex = 'z';
  final copia = ex;
  ex = null;
  print(copia.length);
  print(ex);
}
