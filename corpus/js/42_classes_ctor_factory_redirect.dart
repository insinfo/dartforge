// Factory que devolve subclasse, factory com cache, factory redirecionante, singleton.
abstract class Forma {
  factory Forma(String tipo, double medida) {
    if (tipo == 'quadrado') return Quadrado(medida);
    if (tipo == 'circulo') return Circulo(medida);
    throw ArgumentError('tipo desconhecido: $tipo');
  }

  Forma._();

  String get nome;
  double get area;
}

class Quadrado extends Forma {
  final double lado;
  Quadrado(this.lado) : super._();
  @override
  String get nome => 'quadrado';
  @override
  double get area => lado * lado;
}

class Circulo extends Forma {
  final double raio;
  Circulo(this.raio) : super._();
  @override
  String get nome => 'circulo';
  @override
  double get area => 3 * raio * raio;
}

class Simbolo {
  static final Map<String, Simbolo> _cache = {};
  final String nome;

  Simbolo._(this.nome);

  factory Simbolo(String nome) {
    return _cache.putIfAbsent(nome, () {
      print('criando $nome');
      return Simbolo._(nome);
    });
  }

  static int get tamanhoCache => _cache.length;

  @override
  String toString() => '#$nome';
}

abstract class Animal {
  factory Animal() = Cachorro;
  factory Animal.gato() = Gato;
  String fala();
}

class Cachorro implements Animal {
  @override
  String fala() => 'au';
}

class Gato implements Animal {
  @override
  String fala() => 'miau';
}

class Config {
  static Config? _instancia;
  int leituras = 0;

  Config._();

  factory Config() {
    _instancia ??= Config._();
    return _instancia!;
  }

  static Config get instancia => Config();
}

class Log {
  static final Log unico = Log._();
  final List<String> linhas = [];
  Log._();
  void escreve(String s) => linhas.add(s);
}

void main() {
  final q = Forma('quadrado', 2.5);
  print(q.nome);
  print(q.area);
  print(q is Quadrado);
  final c = Forma('circulo', 1.5);
  print(c.nome);
  print(c.area);
  print(c is Circulo);
  try {
    Forma('triangulo', 1.5);
  } on ArgumentError catch (e) {
    print('erro: ${e.message}');
  }

  final s1 = Simbolo('a');
  final s2 = Simbolo('b');
  final s3 = Simbolo('a');
  print(identical(s1, s3));
  print(identical(s1, s2));
  print(Simbolo.tamanhoCache);
  print([s1, s2, s3]);

  print(Animal().fala());
  print(Animal.gato().fala());
  print(Animal() is Cachorro);
  print(Animal.gato() is Animal);

  final cfg1 = Config();
  final cfg2 = Config();
  cfg1.leituras++;
  cfg2.leituras++;
  print(identical(cfg1, cfg2));
  print(cfg1.leituras);
  print(Config.instancia.leituras);

  Log.unico.escreve('x');
  Log.unico.escreve('y');
  print(Log.unico.linhas);
  print(identical(Log.unico, Log.unico));
}
