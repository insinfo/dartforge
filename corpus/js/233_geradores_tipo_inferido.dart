// Tipo de retorno inferido de funções literais e locais geradoras
// (`sync*`/`async*`): o elemento é o limite superior do que `yield` produz
// (`Null` sem nenhum `yield`); com tipo de contexto, a regra do CFE compara o
// tipo embrulhado com o elemento do contexto e, se não for subtipo, usa o
// contexto. A função local sem tipo escrito passa a ter o tipo inferido.
import 'dart:async';

void main() {
  f() sync* {
    yield 1;
    return;
  }

  g() sync* {
    yield 1;
    yield* [2.5];
  }

  h() async* {
    yield 'a';
    yield* Stream.value(3);
  }

  k() sync* {}
  var l = () sync* {
    yield null;
  };
  var m = () async* {
    yield 1;
  };
  n() sync* {
    yield* <String>[];
  }

  print(f.runtimeType);
  print(g.runtimeType);
  print(h.runtimeType);
  print(k.runtimeType);
  print(l.runtimeType);
  print(m.runtimeType);
  print(n.runtimeType);
  print(f is Iterable<int> Function());
  print(f().toList());

  int x = 1;
  Iterable<num> Function() a = () sync* {
    yield x;
  };
  Iterable<Comparable<num>> Function() b = () sync* {
    yield x;
  };
  Iterable<Object?> Function() c = () sync* {
    yield x;
  };
  Iterable<int?> Function() d = () sync* {
    yield x;
  };
  Iterable<Object> Function() e = () sync* {
    yield x;
  };
  Iterable<String> Function() i = () sync* {};
  Iterable<num>? Function() j = () sync* {
    yield 1;
  };
  Stream<num> Function() s = () async* {
    yield 1;
  };
  dynamic Function() t = () sync* {
    yield 1;
  };
  print(a.runtimeType);
  print(b.runtimeType);
  print(c.runtimeType);
  print(d.runtimeType);
  print(e.runtimeType);
  print(i.runtimeType);
  print(j.runtimeType);
  print(s.runtimeType);
  print(t.runtimeType);

  // A função local usada depois tem o tipo inferido.
  u() sync* {
    yield 1;
    yield 'a';
  }

  var v = u();
  print(v.runtimeType);
  w() sync* {
    yield* v;
    yield* u();
  }

  print(w.runtimeType);
  List<int> Function() lista() => () => [1];
  var z = lista();
  print(z().runtimeType);
  // `yield` recebe o elemento como contexto.
  Iterable<List<num>> Function() ctx = () sync* {
    yield [];
  };
  print(ctx().first.runtimeType);
}
