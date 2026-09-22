// closures com estado: fábrica de contadores, estado compartilhado, closure em campo de classe, closure de closure.
int Function() fabricaContador([int inicio = 0]) {
  var n = inicio;
  return () => ++n;
}

({int Function() incrementa, int Function() decrementa, int Function() valor}) contadorCompleto() {
  var n = 0;
  return (
    incrementa: () => ++n,
    decrementa: () => --n,
    valor: () => n,
  );
}

class Botao {
  final String nome;
  final void Function() aoClicar;
  int cliques = 0;
  Botao(this.nome, this.aoClicar);
  void clica() {
    cliques++;
    aoClicar();
  }
}

class Acumulador {
  int total = 0;
  late final void Function(int) adiciona = (v) => total += v;
  late final int Function() lerTotal = () => total;
}

int Function(int) Function(int) somadorDeSomador(int a) {
  return (b) {
    var chamadas = 0;
    return (c) {
      chamadas++;
      return a + b + c + chamadas * 1000;
    };
  };
}

void main() {
  final c1 = fabricaContador();
  final c2 = fabricaContador(10);
  print(c1());
  print(c1());
  print(c2());
  print(c1());
  print(c2());

  // closures compartilhando o mesmo estado
  final cc = contadorCompleto();
  cc.incrementa();
  cc.incrementa();
  cc.incrementa();
  cc.decrementa();
  print(cc.valor());
  final cc2 = contadorCompleto();
  print(cc2.valor());
  print(cc.valor());

  // closure em campo de classe capturando variável local
  var registro = <String>[];
  final b = Botao('ok', () => registro.add('clicado'));
  b.clica();
  b.clica();
  print('${b.cliques} $registro');

  // closure em campo late capturando this
  final acc = Acumulador();
  acc.adiciona(5);
  acc.adiciona(7);
  print(acc.lerTotal());
  print(acc.total);
  final add = acc.adiciona;
  add(100);
  print(acc.total);

  // closure retornada de closure com estado em cada nível
  final s1 = somadorDeSomador(1);
  final s12 = s1(2);
  print(s12(3));
  print(s12(3));
  final s15 = s1(5);
  print(s15(0));

  // duas closures sobre a mesma variável; uma escreve, outra lê
  var compartilhada = 'a';
  final escreve = (String v) => compartilhada = v;
  final le = () => compartilhada;
  print(le());
  escreve('b');
  print(le());

  // lista de closures cada uma com o próprio estado
  final geradores = List.generate(3, (i) => fabricaContador(i * 100));
  print(geradores.map((g) => g()).toList());
  print(geradores.map((g) => g()).toList());
  print(geradores[1]());

  // closure capturando e alterando parâmetro
  int Function() reduz(int v) {
    return () => v -= 3;
  }

  final r = reduz(10);
  print(r());
  print(r());

  // closure memoizadora
  int Function(int) memoiza(int Function(int) f) {
    final cache = <int, int>{};
    var calculos = 0;
    return (x) {
      if (!cache.containsKey(x)) {
        calculos++;
        cache[x] = f(x);
      }
      print('cálculos até agora: $calculos');
      return cache[x]!;
    };
  }

  final quad = memoiza((x) => x * x);
  print(quad(4));
  print(quad(4));
  print(quad(5));

  // gerador de ids com prefixo
  String Function() geraId(String prefixo) {
    var seq = 0;
    return () => '$prefixo${++seq}';
  }

  final idA = geraId('a');
  final idB = geraId('b');
  print([idA(), idA(), idB(), idA(), idB()]);
}
