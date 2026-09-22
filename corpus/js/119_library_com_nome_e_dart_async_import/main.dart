// library com nome, import de SDK com prefixo (dart:async as asy), dart:math show max, mesma lib importada duas vezes com prefixos diferentes, import com show de SDK.
library minha.lib;

import 'dart:async' as asy;
import 'dart:math' show max, min, pi;
import 'dart:convert' as conv show jsonEncode;
import 'dart:collection' hide HashMap, HashSet;
import 'util.dart' as u1;
import 'util.dart' as u2;
import 'util.dart' show Ponto, origem;

Future<int> viaPrefixo() async {
  final c = asy.Completer<int>();
  asy.Timer(const Duration(milliseconds: 10), () => c.complete(7));
  return c.future;
}

asy.Stream<int> gera() async* {
  yield 1;
  yield 2;
}

Future<void> main() async {
  print(max(3, 9));
  print(min(3, 9));
  print(pi.toStringAsFixed(4));
  print(await viaPrefixo());
  print(await gera().toList());
  final ctrl = asy.StreamController<String>();
  final f = ctrl.stream.toList();
  ctrl.add('a');
  ctrl.add('b');
  await ctrl.close();
  print(await f);
  asy.scheduleMicrotask(() => print('microtask via prefixo'));
  await asy.Future.delayed(const Duration(milliseconds: 5));
  print(await asy.Future.value(3));
  print(asy.Future<int>.value(1) is Future<int>);
  print(asy.Future<int>.value(1) is asy.Future<int>);
  print(conv.jsonEncode({'k': [1, 2]}));
  final q = Queue<int>.from([1, 2, 3]);
  q.addFirst(0);
  print(q);
  final sm = SplayTreeMap<String, int>()..['b'] = 2..['a'] = 1;
  print(sm);
  print('--');
  print(u1.proximo());
  print(u2.proximo());
  print(u1.contador);
  print(u2.contador == u1.contador);
  u2.contador = 100;
  print(u1.contador);
  print(u1.maximo([4, 9, 2]));
  print(u2.Ponto(3, 4).distancia().toStringAsFixed(1));
  print(Ponto(1, 2));
  print(origem);
  print(identical(u1.origem, u2.origem));
  print(identical(u1.origem, origem));
  print(u1.Ponto(1, 1) is u2.Ponto);
  print(u1.Ponto == u2.Ponto);
  print(Ponto == u1.Ponto);
  print(const u1.Ponto(5, 5) == const u2.Ponto(5, 5));
  print(identical(const u1.Ponto(5, 5), const u2.Ponto(5, 5)));
  print('fim');
}
