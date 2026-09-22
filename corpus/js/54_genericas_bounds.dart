// Classes genéricas: bounds (extends num, Comparable), métodos genéricos, covariância, dois parâmetros, factory genérica.
class Caixa<T> {
  T valor;
  Caixa(this.valor);

  Caixa<R> mapa<R>(R Function(T) f) => Caixa<R>(f(valor));

  bool mesmoTipo<U>(Caixa<U> outra) => this is Caixa<U> && outra is Caixa<T>;

  @override
  String toString() => 'Caixa($valor)';
}

class Somador<T extends num> {
  final List<T> itens = [];
  void add(T t) => itens.add(t);
  T get soma => itens.fold<T>(itens.first, (a, b) => (a + b) as T);
  T get maior => itens.reduce((a, b) => a > b ? a : b);
}

class Ordenado<T extends Comparable<Object>> {
  final List<T> _itens = [];
  void insere(T t) {
    var i = 0;
    while (i < _itens.length && _itens[i].compareTo(t) < 0) {
      i++;
    }
    _itens.insert(i, t);
  }

  T get minimo => _itens.first;
  T get maximo => _itens.last;
  List<T> get lista => _itens;
}

class Par<K, V> {
  final K chave;
  final V valor;
  const Par(this.chave, this.valor);
  Par<V, K> inverte() => Par(valor, chave);
  @override
  String toString() => '$chave=>$valor';
}

class Versao implements Comparable<Versao> {
  final int maior;
  final int menor;
  Versao(this.maior, this.menor);
  @override
  int compareTo(Versao o) =>
      maior != o.maior ? maior.compareTo(o.maior) : menor.compareTo(o.menor);
  @override
  String toString() => '$maior.$menor';
}

abstract class Repositorio<T> {
  factory Repositorio() = RepositorioMemoria<T>;
  void salva(T t);
  List<T> todos();
}

class RepositorioMemoria<T> implements Repositorio<T> {
  final List<T> _dados = [];
  @override
  void salva(T t) => _dados.add(t);
  @override
  List<T> todos() => List.of(_dados);
}

T primeiro<T>(List<T> l) => l[0];

List<T> repete<T>(T t, int n) => List.filled(n, t);

void main() {
  final ci = Caixa<int>(3);
  final cs = Caixa('x');
  print(ci);
  print(cs);
  print(ci.mapa((v) => v * 2));
  print(ci.mapa((v) => 'v$v'));
  print(cs.mapa((s) => s.length));
  print(ci.runtimeType);
  print(cs.runtimeType);
  print(Caixa(1.5).runtimeType);
  print(Caixa<num>(1).runtimeType);
  final Caixa dinamica = Caixa<int>(1);
  print(dinamica.runtimeType);
  print(Caixa(null).runtimeType);

  print(ci is Caixa<num>);
  print(ci is Caixa<Object>);
  print(ci is Caixa<String>);
  print(ci is Caixa);
  print(Caixa<num>(1) is Caixa<int>);
  print(ci.mesmoTipo(Caixa<int>(9)));
  print(ci.mesmoTipo(Caixa<num>(9)));
  final Caixa<Object> obj = ci;
  print(obj.valor);
  final Caixa<dynamic> din = Caixa<String>('d');
  print(din.valor);

  final s = Somador<int>();
  s.add(3);
  s.add(9);
  s.add(4);
  print(s.soma);
  print(s.maior);
  final sd = Somador<double>();
  sd.add(0.5);
  sd.add(0.25);
  print(sd.soma);
  print(sd.maior);

  final o = Ordenado<int>();
  for (final v in [5, 1, 4, 2]) {
    o.insere(v);
  }
  print(o.lista);
  print(o.minimo);
  print(o.maximo);
  final os = Ordenado<String>();
  os.insere('pera');
  os.insere('abacate');
  os.insere('manga');
  print(os.lista);
  final ov = Ordenado<Versao>();
  ov.insere(Versao(2, 1));
  ov.insere(Versao(1, 9));
  ov.insere(Versao(2, 0));
  print(ov.lista);
  print(ov.maximo);

  const p = Par('a', 1);
  print(p);
  print(p.inverte());
  print(p.inverte().inverte().chave);
  print(p.runtimeType);
  print(Par(1, [2]).runtimeType);

  final repo = Repositorio<String>();
  repo.salva('um');
  repo.salva('dois');
  print(repo.todos());
  print(repo is RepositorioMemoria<String>);
  print(repo.runtimeType);

  print(primeiro([7, 8]));
  print(primeiro(['q']));
  print(repete('ab', 3));
  print(repete<int?>(null, 2));
}
