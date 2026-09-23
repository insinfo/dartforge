// Encaminhadores de noSuchMethod: muitos membros abstratos vindos da
// superclasse, de mixins e de interfaces. A ordem em que o emissor os escreve
// tem de ser a mesma em toda execução (tests/determinismo.rs).
abstract class Forma {
  double area();
  double perimetro();
  String get nome;
  set nome(String v);
  int get lados;
}

abstract class Colorida {
  String get cor;
  set cor(String v);
  String pinta(String tinta, bool brilho);
}

mixin Registro {
  List<String> get historico;
  void registra(String evento);
}

mixin Contagem {
  int get total;
  int soma(int a, int b);
}

abstract class Base {
  int contador = 0;
  int incrementa();
  int get dobro;
}

class Fantasma extends Base with Registro, Contagem implements Forma, Colorida {
  final List<String> chamadas = [];

  final Map<Symbol, String> _nomes = {
    #area: 'area',
    #perimetro: 'perimetro',
    #nome: 'nome',
    const Symbol('nome='): 'nome',
    #lados: 'lados',
    #cor: 'cor',
    const Symbol('cor='): 'cor',
    #pinta: 'pinta',
    #historico: 'historico',
    #registra: 'registra',
    #total: 'total',
    #soma: 'soma',
    #incrementa: 'incrementa',
    #dobro: 'dobro',
  };

  final Map<String, Object?> _respostas = {
    'area': 2.25,
    'perimetro': 10.5,
    'nome': 'fantasma',
    'lados': 3,
    'cor': 'azul',
    'pinta': 'pintado',
    'historico': <String>['criado'],
    'registra': null,
    'total': 42,
    'soma': 7,
    'incrementa': 1,
    'dobro': 2,
  };

  @override
  dynamic noSuchMethod(Invocation i) {
    final n = _nomes[i.memberName] ?? '?';
    final tipo = i.isGetter
        ? 'get'
        : i.isSetter
            ? 'set'
            : 'metodo';
    final nomeados = i.namedArguments.entries.map((e) => '${_nomes[e.key] ?? 'brilho'}=${e.value}').join(',');
    chamadas.add('$tipo $n ${i.positionalArguments} {$nomeados}');
    return _respostas[n];
  }
}

void main() {
  final f = Fantasma();
  print(f.area());
  print(f.perimetro());
  print(f.nome);
  f.nome = 'outro';
  print(f.lados);
  print(f.cor);
  f.cor = 'verde';
  print(f.pinta('tinta', true));
  print(f.historico);
  f.registra('evento');
  print(f.total);
  print(f.soma(3, 4));
  print(f.incrementa());
  print(f.dobro);
  print(f.contador);
  for (final c in f.chamadas) {
    print(c);
  }
  final Forma forma = f;
  final Colorida colorida = f;
  print('${forma.area() + forma.perimetro()} ${colorida.pinta('x', false)}');
  print(f.chamadas.length);
}
