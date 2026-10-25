// A mensagem do `AssertionError` como a da VM (trecho da condição, URI,
// linha e coluna); constantes canônicas pelo conteúdo, como a VM canonicaliza:
// instâncias de construtores diferentes com os mesmos campos, listas com
// spreads e casts, records `const`, interpolações de constantes (também fora
// de contexto constante) e criações cujo argumento não tem chave estática;
// deslocamentos de `int` com contagem de 64 ou mais (0, ou o sinal) e negativa
// (`ArgumentError`), estáticos e dinâmicos; `r.$2` de um record cujo `$2` é
// um campo nomeado.
const N = 8;
const MIN = 2 - 1;
const MAX = N * 2;
const MASK = (1 << (MAX - MIN + 1)) - 1;
const s = 'MIN = $MIN  MAX = $MAX  MASK = $MASK';

class C {
  final int x;
  const C(this.x);
  const C.checked(this.x, int y) : assert(x < y, 'x < y');
  const C.other(int y) : x = y - 1;
}

class Foo {
  final x;
  const Foo(this.x);
}

const dynamic list = [1, 2, 3, 4];
const dynamic set = {1, 2, 3, 4};

int id(int x) => int.parse('$x');

String message(void Function() f) {
  try {
    f();
  } catch (e) {
    final text = e.toString();
    // The URI of this file varies with the checkout.
    return text.substring(text.indexOf(': Failed'));
  }
  return 'no error';
}

void main() {
  print(message(() {
    assert(id(1) == 2);
  }));
  print(message(() {
    assert(id(1) > 2, 'one is not greater');
  }));
  print(message(() => C.checked(id(3), 2)));

  const c1 = C(1);
  print(identical(c1, const C.checked(1, 2)));
  print(identical(c1, const C.other(2)));
  print(identical(c1, const C(2)));

  print(MASK);
  print(identical(s, 'MIN = $MIN  MAX = $MAX  MASK = $MASK'));
  print(identical(const Foo(s), const Foo('MIN = $MIN  MAX = $MAX  MASK = $MASK')));
  print(identical(const Foo(MASK), const Foo(s)));

  print(identical(list, const <int>[...list]));
  print(identical(list, const <int>[...set]));
  print(identical(list, const <int>[1, ...<int>[2, 3], 4]));
  const list23 = <int>[2, 3] as List<int>?;
  const list4 = <int>[4] as List<int>?;
  print(identical(list, const <int>[1, ...?list23, ...?(null), ...?list4]));
  const l0 = <Object?>[null, null];
  const l1 = <Object?>[l0, l0];
  const m0 = <Object?>[null, null];
  const m1 = <Object?>[m0, m0];
  print(identical(l1, m1));

  const r = (1, 2);
  print(identical(r, const (1, 2)));
  print(identical(const (1, x: 2), const (1, x: 2)));
  print(identical(const (1, x: 2), const (1, x: 3)));

  final x = id(499);
  for (final n in [0, 1, 33, 63, 64, 65, 100, 1 << 40]) {
    print('$n: ${x << n} ${x >> n} ${x >>> n} ${-x >> n} ${-x >>> n}');
  }
  print('${x << 64} ${x >> 70} ${-x >> 70} ${x >>> 64}');
  dynamic d = x;
  for (dynamic n in [33, 64, 65]) {
    print('$n: ${d << n} ${d >> n} ${d >>> n}');
  }
  for (final f in [() => x << id(-1), () => x >> id(-1), () => d << -1]) {
    try {
      print(f());
    } on ArgumentError catch (e) {
      print(e);
    }
  }

  dynamic record = (10, $2: 20, $1000000000000000000: 'meow');
  print(record.$1);
  print(record.$2);
  print(record.$1000000000000000000);
}
