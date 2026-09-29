// Vários `await` numa mesma expressão: o valor de cada `await` que
// atravessa o próximo ponto de suspensão mora no quadro
// (`lower/async_sm.rs`, `await_valor` + `guardar_vivos`), em lista, mapa,
// conjunto, record, argumentos posicionais e nomeados, operadores
// binários, interpolação, cascata, condicional e dentro de `async*`. E o
// `runtimeType` de um `_Future`, que a VM mostra como `Future`
// (`lower/rti.rs`, `nome_visivel`). A saída tem de ser a da VM.
import 'dart:async';

Future<int> g(int n) async {
  await null;
  return n * 2;
}

Future<String> s(String x) async {
  await Future<void>.delayed(Duration.zero);
  return x;
}

Future<int> falha() async {
  await null;
  throw StateError('falhou');
}

String junta(int a, String b, int c, {int d = 0, String e = '-'}) => '$a|$b|$c|$d|$e';

class Acumulador {
  final List<Object?> itens = [];
  void add(Object? x) => itens.add(x);
  @override
  String toString() => 'Acumulador$itens';
}

class Par {
  final int a;
  final int b;
  Par(this.a, this.b);
  Future<int> soma(int extra) async => a + b + (await g(extra));
}

Stream<String> gerador() async* {
  yield '${await g(1)}-${await s('x')}-${await g(2)}';
  yield* Stream.fromIterable([await g(3), await g(4)].map((v) => '$v'));
  final l = [await g(5), await s('y'), await g(6)];
  yield '$l';
}

Iterable<int> sincrono() sync* {
  var x = 1;
  yield x + 1;
  yield x + 2;
}

Future<void> main() async {
  final c = Completer<String>()..complete('ok');
  print([await Future.value(3), await c.future, await g(5)]);
  print({'a': await g(1), await s('b'): await g(2), 'c': await s('C')});
  print({await g(1), await g(2), await g(3)});
  print((await g(1), await s('dois'), nome: await g(3)));
  print(junta(await g(1), await s('b'), await g(3)));
  print(junta(await g(1), await s('b'), await g(3), e: await s('E'), d: await g(4)));
  print(await g(1) + await g(2) * await g(3));
  print(await g(10) - await g(3));
  print(await s('a') + await s('b') + await s('c'));
  print(await g(1) < await g(2) && await g(3) > await g(1));
  print('${await g(1)} ${await s('meio')} ${await g(3)}');
  final a = Acumulador()
    ..add(await g(1))
    ..add(await s('dois'))
    ..add(await g(3));
  print(a);
  print(await g(1) > 0 ? [await s('sim'), await g(7)] : [await s('não')]);
  print(await Par(await g(1), await g(2)).soma(await g(3)));
  final m = <String, List<int>>{
    'x': [await g(1), await g(2)],
    'y': [await g(3), for (var i = 0; i < 2; i++) await g(i)],
  };
  print(m);
  var i = 0;
  final r = [await g(i++), await g(i++), await g(i++)];
  print('$r $i');
  try {
    print([await g(1), await falha(), await g(3)]);
  } on StateError catch (e) {
    print('pegou: ${e.message}');
  }
  final pegos = [
    await g(1),
    await falha().catchError((Object _) => -1),
    await g(3),
  ];
  print(pegos);
  await for (final linha in gerador()) {
    print(linha);
  }
  print(sincrono().toList());
  print(Future.value(3).runtimeType);
  print(g(1).runtimeType);
  print(Completer<String>().future.runtimeType);
  print(Future<int>.delayed(Duration.zero, () => 1).runtimeType);
  print(Completer<int>.sync().runtimeType);
  print(StreamController<int>().stream.runtimeType);
  print(Future.value(3).runtimeType == Future<int>);
  print('${Future<List<int>>.value([1]).runtimeType}');
}
