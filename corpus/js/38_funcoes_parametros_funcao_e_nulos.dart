// parâmetros de tipo função nullable, cb?.call(), default null, Function dinâmico, Never via throw.
void executa(void Function()? cb) {
  cb?.call();
  print('executa: cb ${cb == null ? 'nulo' : 'presente'}');
}

int transforma(int x, {int Function(int)? f}) => f?.call(x) ?? x;

String comFallback(String s, [String Function(String)? t]) {
  final fn = t ?? (v) => v;
  return fn(s);
}

void notifica({void Function(String)? aoSucesso, void Function(Object)? aoErro, bool falha = false}) {
  if (falha) {
    aoErro?.call('erro simulado');
  } else {
    aoSucesso?.call('deu certo');
  }
}

Object chamaDinamico(Function f, int a, int b) => f(a, b);

Never falha(String msg) => throw StateError(msg);

int usaNever(int? v) {
  if (v == null) falha('valor nulo');
  return v * 2;
}

Never semRetorno() {
  throw UnsupportedError('nunca retorna');
}

int? talvezFuncao(int Function()? f) => f?.call();

void main() {
  executa(null);
  executa(() => print('callback chamado'));

  print(transforma(5));
  print(transforma(5, f: (x) => x * 10));
  print(transforma(5, f: null));

  print(comFallback('abc'));
  print(comFallback('abc', (s) => s.toUpperCase()));

  notifica();
  notifica(aoSucesso: (m) => print('sucesso: $m'));
  notifica(aoErro: (e) => print('erro: $e'), falha: true);
  notifica(aoSucesso: (m) => print('não chamado'), falha: true);

  // Function chamada dinamicamente
  print(chamaDinamico((int a, int b) => a + b, 2, 3));
  print(chamaDinamico((num a, num b) => '$a|$b', 2, 3));
  print(chamaDinamico((Object a, Object b) => [a, b], 2, 3));

  // Function com aridade errada lança NoSuchMethodError
  try {
    chamaDinamico((int a) => a, 1, 2);
    print('não lançou');
  } catch (e) {
    print('aridade errada: ${e is NoSuchMethodError}');
  }

  // Never
  print(usaNever(21));
  try {
    usaNever(null);
  } on StateError catch (e) {
    print('Never: ${e.message}');
  }
  try {
    semRetorno();
  } on UnsupportedError catch (e) {
    print(e.message);
  }

  // Never em expressão: o tipo do ?? ainda é int
  int? nulo;
  try {
    final int r = nulo ?? falha('sem valor');
    print(r);
  } on StateError catch (e) {
    print('?? com Never: ${e.message}');
  }

  // variável de função nullable
  int Function(int)? op;
  print(op?.call(3));
  print(op == null);
  op = (x) => x + 1;
  print(op?.call(3));
  print(op(3));
  print(talvezFuncao(null));
  print(talvezFuncao(() => 7));

  // lista de callbacks nullable
  final cbs = <void Function()?>[() => print('cb0'), null, () => print('cb2')];
  for (final cb in cbs) {
    cb?.call();
  }
  print(cbs.whereType<void Function()>().length);

  // função que aceita Function e testa o tipo antes de chamar
  String descreve(Function f) {
    if (f is int Function(int)) return 'int->int: ${f(1)}';
    if (f is String Function()) return 'String(): ${f()}';
    return 'outra';
  }

  print(descreve((int x) => x * 3));
  print(descreve(() => 's'));
  print(descreve((int a, int b) => a));

  // default null vs default função
  int aplicaOuDobra(int x, [int Function(int)? f]) => (f ?? (v) => v * 2)(x);
  print(aplicaOuDobra(4));
  print(aplicaOuDobra(4, (v) => v - 1));

  // Function.apply com nomeados
  int nomeados(int a, {int b = 0}) => a + b;
  print(Function.apply(nomeados, [1], {#b: 2}));
  print(Function.apply(nomeados, [1]));
}
