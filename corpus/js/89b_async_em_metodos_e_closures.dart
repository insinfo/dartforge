// async em métodos de classe, getters Future, closures async em forEach (não aguardadas) vs for com await, Future<void> vs void async, arrow, await em expressão e de não-Future.
import 'dart:async';

class Contador {
  int valor = 0;
  final String nome;
  Contador(this.nome);

  Future<int> incrementa() async {
    await Future.delayed(Duration(milliseconds: 5));
    valor++;
    return valor;
  }

  Future<String> get descricao async => '$nome=$valor';

  Future<int> dobro() async => valor * 2;

  static Future<Contador> cria(String nome) async {
    final c = Contador(nome);
    await c.incrementa();
    return c;
  }

  Future<void> reset() async {
    valor = 0;
  }
}

Future<int> lento(int v) async {
  await Future.delayed(Duration(milliseconds: 5));
  return v;
}

Future<void> comVoidFuture() async {
  await null;
  print('Future<void> terminou');
}

void voidAsync(List<String> log) async {
  log.add('void async antes');
  await null;
  log.add('void async depois');
}

Future<void> main() async {
  final c = await Contador.cria('c');
  print(await c.descricao);
  print('incrementa: ${await c.incrementa()}');
  print('dobro: ${await c.dobro()}');
  await c.reset();
  print(await c.descricao);
  print('--');
  final ordem = <String>[];
  [1, 2, 3].forEach((i) async {
    ordem.add('início $i');
    await lento(i);
    ordem.add('fim $i');
  });
  ordem.add('forEach voltou');
  await Future.delayed(Duration(milliseconds: 30));
  print('forEach: $ordem');
  final ordem2 = <String>[];
  for (final i in [1, 2, 3]) {
    ordem2.add('início $i');
    await lento(i);
    ordem2.add('fim $i');
  }
  ordem2.add('for voltou');
  print('for: $ordem2');
  print('--');
  final log = <String>[];
  voidAsync(log);
  log.add('chamada voltou');
  await null;
  await null;
  print('void async: $log');
  await comVoidFuture();
  print('--');
  final soma = (await lento(2)) + (await lento(3)) * 10;
  print('await em expressão: $soma');
  final lista = [await lento(1), await lento(2)];
  print('await em literal: $lista');
  final mapa = {'a': await lento(7)};
  print('await em mapa: $mapa');
  print('await de int: ${await 5}');
  print('await de string: ${await 'texto'}');
  final Object o = 'objeto';
  print('await de Object: ${await o}');
  final dynamic d = lento(4);
  print('await de dynamic Future: ${await d}');
  final FutureOr<int> fo = 8;
  print('await de FutureOr: ${await fo}');
  print('await em condição: ${(await lento(1)) == 1 ? 'sim' : 'não'}');
  print('await em interpolação: ${await lento(9)}');
  final Future<int> Function(int) arrow = (x) async => x + 1;
  print('closure async arrow: ${await arrow(1)}');
  final closure = () async {
    final a = await lento(10);
    return a * 2;
  };
  print('closure async bloco: ${await closure()}');
  final fs = [1, 2, 3].map((i) => lento(i * 10)).toList();
  print('map para futures depois wait: ${await Future.wait(fs)}');
  var acumulado = 0;
  await for (final v in Stream.fromIterable([1, 2, 3]).asyncMap(lento)) {
    acumulado += v;
  }
  print('acumulado: $acumulado');
  print('fim');
}
