// exceções dentro de closures: forEach, map preguiçoso (só ao iterar), comparator de sort, callbacks; laço que captura por iteração.
class Item {
  final String nome;
  final int? peso;
  Item(this.nome, this.peso);
}

void executa(void Function() cb) {
  print('antes do callback');
  cb();
  print('depois do callback');
}

int aplica(int Function(int) f, int v) => f(v);

void main() {
  // forEach: lança na hora
  try {
    [1, 2, 3].forEach((x) {
      print('forEach $x');
      if (x == 2) throw StateError('parou no $x');
    });
  } on StateError catch (e) {
    print(e.message);
  }

  // map preguiçoso: não lança até iterar
  final mapeado = [1, 2, 3].map((x) {
    if (x == 3) throw 'falha no map $x';
    return x * 10;
  });
  print('map criado sem lançar');
  try {
    print(mapeado.toList());
  } catch (e) {
    print(e);
  }
  print(mapeado.first);
  print(mapeado.take(2).toList());
  try {
    mapeado.length;
    print('length não avalia o map');
  } catch (e) {
    print('length lançou');
  }
  try {
    print(mapeado.last);
  } catch (e) {
    print('last: $e');
  }

  // where preguiçoso
  final filtrado = [1, 2].where((x) => x == 2 ? throw 'no where' : true);
  print('where criado');
  try {
    filtrado.toList();
  } catch (e) {
    print(e);
  }

  // comparator de sort que lança: lista pode ficar parcialmente ordenada; não a imprimimos
  final itens = [Item('a', 3), Item('b', null), Item('c', 1)];
  try {
    itens.sort((x, y) => x.peso!.compareTo(y.peso!));
  } catch (e) {
    print('sort lançou: ${e is TypeError}');
  }
  final validos = itens.where((i) => i.peso != null).toList();
  validos.sort((x, y) => x.peso!.compareTo(y.peso!));
  print(validos.map((i) => i.nome).toList());

  // closure passada a função: a exceção atravessa o chamador
  try {
    executa(() => throw 'do callback');
  } catch (e) {
    print('capturou $e');
  }
  try {
    print(aplica((v) => v ~/ (v - 5), 5));
  } catch (e) {
    print('divisão em closure lançou');
  }
  print(aplica((v) => v ~/ (v - 3), 5));

  // laço que captura por iteração e continua
  final entradas = ['1', 'x', '3', '', '5'];
  var soma = 0;
  final erros = <String>[];
  for (final s in entradas) {
    try {
      soma += int.parse(s);
    } on FormatException {
      erros.add(s.isEmpty ? '(vazio)' : s);
      continue;
    }
    print('somou $s');
  }
  print('soma $soma erros $erros');

  // fold com exceção no meio: acumulador anterior não é visível
  try {
    [1, 2, 3].fold<int>(0, (acc, x) => x == 2 ? throw 'fold' : acc + x);
  } catch (e) {
    print(e);
  }

  // expand com exceção
  try {
    [1, 2].expand((x) => x == 2 ? throw 'expand' : [x]).toList();
  } catch (e) {
    print(e);
  }

  // any/every param quando lança
  var visitados = 0;
  try {
    [1, 2, 3].any((x) {
      visitados++;
      if (x == 2) throw 'any';
      return false;
    });
  } catch (e) {
    print('$e visitados $visitados');
  }

  // exceção em closure aninhada dentro de laço com try interno
  final resultados = <String>[];
  for (var i = 0; i < 4; i++) {
    final calc = () => i.isOdd ? throw ArgumentError('ímpar $i') : i * 2;
    try {
      resultados.add('${calc()}');
    } on ArgumentError catch (e) {
      resultados.add('erro(${e.message})');
    }
  }
  print(resultados);

  // exceção em closure guardada e chamada depois
  final adiada = () => throw 'adiada';
  print('closure criada');
  try {
    adiada();
  } catch (e) {
    print(e);
  }

  // removeWhere / retainWhere com exceção deixam a lista intacta? não garantido; só capturamos
  final l = [1, 2, 3];
  try {
    l.removeWhere((x) => x == 2 ? throw 'removeWhere' : false);
  } catch (e) {
    print(e);
  }
  print(l.contains(1));

  // map em Map com exceção
  try {
    <String, int>{'a': 1}.map<String, int>((k, v) => throw 'map de Map');
  } catch (e) {
    print(e);
  }

  // List.generate com exceção no gerador
  try {
    List.generate(3, (i) => i == 1 ? throw 'generate $i' : i);
  } catch (e) {
    print(e);
  }

  // while com contador de tentativas e exceção até a terceira
  var tentativas = 0;
  while (true) {
    try {
      tentativas++;
      if (tentativas < 3) throw 'tentativa $tentativas falhou';
      print('sucesso na tentativa $tentativas');
      break;
    } catch (e) {
      print(e);
    }
  }
  print('fim');
}
