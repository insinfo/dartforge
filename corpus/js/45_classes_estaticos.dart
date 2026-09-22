// Membros estáticos: campos com lazy init, métodos, constantes, contador de instâncias, static em genérica.
class Registro {
  static int criados = 0;
  static const String prefixo = 'REG';
  static final List<String> nomes = [];
  static late int tardio = _calculaTardio();

  static int _calculaTardio() {
    print('inicializando tardio');
    return 42;
  }

  static String lazyTexto = _geraTexto();

  static String _geraTexto() {
    print('inicializando lazyTexto');
    return 'texto-lazy';
  }

  final int id;
  final String nome;

  Registro(this.nome) : id = ++criados {
    nomes.add(nome);
  }

  static Registro maisRecente() => Registro('auto$criados');

  static String formata(int id) => '$prefixo-${id.toString().padLeft(3, '0')}';

  static int somaIds(List<Registro> rs) {
    var total = 0;
    for (final r in rs) {
      total += r.id;
    }
    return total;
  }

  @override
  String toString() => '${formata(id)}:$nome';
}

class Pilha<T> {
  static int instancias = 0;
  final List<T> _itens = [];

  Pilha() {
    instancias++;
  }

  static Pilha<E> de<E>(List<E> itens) {
    final p = Pilha<E>();
    for (final i in itens) {
      p.empilha(i);
    }
    return p;
  }

  void empilha(T t) => _itens.add(t);
  T desempilha() => _itens.removeLast();
  int get tamanho => _itens.length;
}

class Matematica {
  static const double pi3 = 3.14;
  static const int maximo = 1 << 20;
  static int quadrado(int n) => n * n;
  static int cubo(int n) => n * quadrado(n);
  static int Function(int) get dobrador => (n) => n * 2;
}

void main() {
  print(Registro.criados);
  print(Registro.prefixo);
  final a = Registro('a');
  final b = Registro('b');
  print(a);
  print(b);
  print(Registro.criados);
  print(Registro.maisRecente());
  print(Registro.criados);
  print(Registro.nomes);
  print(Registro.somaIds([a, b]));
  print(Registro.formata(7));

  print('antes de ler tardio');
  print(Registro.tardio);
  print(Registro.tardio);
  print('antes de ler lazyTexto');
  print(Registro.lazyTexto);
  Registro.lazyTexto = 'novo';
  print(Registro.lazyTexto);

  print(Pilha.instancias);
  final p1 = Pilha<int>();
  final p2 = Pilha.de(['x', 'y']);
  print(Pilha.instancias);
  p1.empilha(1);
  print(p1.tamanho);
  print(p2.desempilha());
  print(p2.tamanho);

  print(Matematica.pi3);
  print(Matematica.maximo);
  print(Matematica.quadrado(9));
  print(Matematica.cubo(3));
  print(Matematica.dobrador(21));
  final f = Matematica.quadrado;
  print([1, 2, 3].map(f).toList());
}
