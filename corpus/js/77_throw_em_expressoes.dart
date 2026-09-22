// throw como expressão: em ??, em arrow function, em ternário, em initializer late, em getter, em cascata e argumentos.
class Config {
  final Map<String, String> valores;
  Config(this.valores);

  String operator [](String chave) => valores[chave] ?? (throw ArgumentError('chave ausente: $chave'));

  String get obrigatorio => valores['obrigatorio'] ?? (throw StateError('sem obrigatorio'));

  late final String tardio = valores['tardio'] ?? (throw StateError('sem tardio'));

  late final int contagemInicializacoes = _conta();
  int _chamadas = 0;
  int _conta() {
    _chamadas++;
    if (_chamadas == 1) throw StateError('primeira inicialização falha');
    return _chamadas;
  }
}

int nuncaRetorna(String m) => throw Exception(m);

Never falhaSempre() => throw UnsupportedError('nunca');

String classifica(int n) => n < 0 ? throw RangeError('negativo') : n == 0 ? 'zero' : 'positivo';

void main() {
  // ?? com throw
  String? nulo;
  String? cheio = 'v';
  print(cheio ?? (throw 'não avaliado'));
  try {
    print(nulo ?? (throw ArgumentError('era nulo')));
  } on ArgumentError catch (e) {
    print(e.message);
  }

  // arrow function que lança
  try {
    nuncaRetorna('arrow');
  } catch (e) {
    print(e);
  }
  final f = (int x) => x > 0 ? x : throw 'não positivo: $x';
  print(f(3));
  try {
    f(-1);
  } catch (e) {
    print(e);
  }

  // Never
  try {
    falhaSempre();
  } on UnsupportedError catch (e) {
    print(e.message);
  }

  // ternário
  print(classifica(0));
  print(classifica(5));
  try {
    classifica(-5);
  } on RangeError catch (e) {
    print('RangeError: ${e.message}');
  }

  // operador [] e getter que lançam
  final cfg = Config({'a': '1', 'obrigatorio': 'ok', 'tardio': 't'});
  print(cfg['a']);
  try {
    cfg['zzz'];
  } on ArgumentError catch (e) {
    print(e.message);
  }
  print(cfg.obrigatorio);
  final cfg2 = Config({});
  try {
    cfg2.obrigatorio;
  } on StateError catch (e) {
    print(e.message);
  }

  // initializer late que lança: campo continua não inicializado e tenta de novo
  print(cfg.tardio);
  try {
    cfg2.tardio;
  } on StateError catch (e) {
    print(e.message);
  }
  try {
    cfg2.tardio;
  } on StateError catch (e) {
    print('de novo: ${e.message}');
  }
  try {
    cfg.contagemInicializacoes;
  } on StateError catch (e) {
    print(e.message);
  }
  print(cfg.contagemInicializacoes);
  print(cfg.contagemInicializacoes);

  // throw em argumento: os argumentos anteriores são avaliados
  void tres(int a, int b, int c) => print('$a $b $c');
  int avalia(String s) {
    print('avaliou $s');
    return s.length;
  }

  try {
    tres(avalia('um'), throw 'no segundo argumento', avalia('três'));
  } catch (e) {
    print(e);
  }

  // throw em cascata e em interpolação
  try {
    final l = [1]..add(2)..add(throw 'na cascata');
    print(l);
  } catch (e) {
    print(e);
  }
  try {
    print('a${throw 'b'}c');
  } catch (e) {
    print(e);
  }

  // throw em condição de if e em for
  try {
    if (throw 'na condição') print('nunca');
  } catch (e) {
    print(e);
  }
  try {
    for (var i = 0; i < (i == 2 ? throw 'no limite $i' : 5); i++) {
      print('i=$i');
    }
  } catch (e) {
    print(e);
  }

  // throw em switch expression e em spread
  try {
    final r = switch (3) { 1 => 'um', _ => throw 'sem caso' };
    print(r);
  } catch (e) {
    print(e);
  }
  try {
    final l = [1, ...(throw 'no spread')];
    print(l);
  } catch (e) {
    print(e);
  }

  // throw em expressão de return e em atribuição
  int retorna() => throw 'no return';
  try {
    retorna();
  } catch (e) {
    print(e);
  }
  var x = 1;
  try {
    x = throw 'na atribuição';
  } catch (e) {
    print('$e; x ainda $x');
  }

  // throw de resultado de função
  try {
    throw Exception('de função'.toUpperCase());
  } catch (e) {
    print(e);
  }
  print('fim');
}
