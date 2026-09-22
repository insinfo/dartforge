// Iterable customizado: Iterator próprio, IterableBase, IterableMixin, extends Iterable, uso em for-in/map/where.
import 'dart:collection';

class ContagemIterator implements Iterator<int> {
  final int fim;
  int _atual = -1;
  ContagemIterator(this.fim);

  @override
  int get current => _atual;

  @override
  bool moveNext() {
    if (_atual + 1 >= fim) return false;
    _atual++;
    return true;
  }
}

class Contagem extends IterableBase<int> {
  final int fim;
  const Contagem(this.fim);
  @override
  Iterator<int> get iterator => ContagemIterator(fim);
}

class Pares with IterableMixin<int> {
  final int limite;
  Pares(this.limite);
  @override
  Iterator<int> get iterator => Iterable.generate(limite, (i) => i * 2).iterator;
}

class Palavras extends Iterable<String> {
  final String texto;
  Palavras(this.texto);
  @override
  Iterator<String> get iterator => texto.split(' ').where((w) => w.isNotEmpty).iterator;
}

class Fibonacci extends IterableBase<int> {
  final int quantos;
  Fibonacci(this.quantos);
  @override
  Iterator<int> get iterator => _FibIterator(quantos);
}

class _FibIterator implements Iterator<int> {
  final int quantos;
  int _a = 0, _b = 1, _n = 0, _atual = 0;
  _FibIterator(this.quantos);
  @override
  int get current => _atual;
  @override
  bool moveNext() {
    if (_n >= quantos) return false;
    _atual = _a;
    final prox = _a + _b;
    _a = _b;
    _b = prox;
    _n++;
    return true;
  }
}

class Pilha<T> extends IterableBase<T> {
  final List<T> _itens = [];
  void empilha(T v) => _itens.add(v);
  T desempilha() => _itens.removeLast();
  @override
  Iterator<T> get iterator => _itens.reversed.iterator;
}

void main() {
  final c = Contagem(4);
  for (final x in c) {
    print('contagem $x');
  }
  print(c.toList());
  print(c.length);
  print(c.first);
  print(c.last);
  print(c.map((x) => x * x).toList());
  print(c.where((x) => x.isOdd).toList());
  print(c.contains(2));
  print(c.contains(9));
  print(c.isEmpty);
  print(Contagem(0).isEmpty);
  print(c.fold<int>(0, (a, b) => a + b));
  print(c.join('-'));
  print(c.skip(2).toList());
  print(c.take(2).toList());
  print(c.elementAt(3));
  print(c.reduce((a, b) => a + b));
  print(c is Iterable<int>);
  print(c is List);

  // cada chamada a iterator gera novo iterador independente
  final it1 = c.iterator;
  final it2 = c.iterator;
  it1.moveNext();
  it1.moveNext();
  it2.moveNext();
  print('${it1.current} ${it2.current}');

  final p = Pares(5);
  print(p.toList());
  print(p.any((x) => x == 6));
  print(p.every((x) => x.isEven));
  print(p.toSet());
  print(p.length);

  final w = Palavras('  o rato  roeu a roupa ');
  print(w.toList());
  print(w.length);
  print(w.map((s) => s.length).toList());
  print(w.where((s) => s.startsWith('r')).join(','));
  for (final (i, s) in w.indexed) {
    print('$i:$s');
  }

  final fib = Fibonacci(10);
  print(fib.toList());
  print(fib.last);
  print(fib.where((x) => x.isEven).toList());
  print(fib.takeWhile((x) => x < 10).toList());
  print(fib.expand((x) => [x, x]).take(6).toList());

  final pilha = Pilha<String>();
  pilha.empilha('a');
  pilha.empilha('b');
  pilha.empilha('c');
  print(pilha.toList());
  print(pilha.desempilha());
  print(pilha.toList());
  print(pilha.first);
  print([...pilha]);
  print({for (final x in pilha) x: x.codeUnitAt(0)});

  // Iterable customizado passado onde se espera Iterable
  print(List.of(c));
  print(Set.of(p));
  print([0, ...c, ...p]);
  print(c.followedBy(p).toList());

  // iterator manual sobre o custom
  final it = Fibonacci(3).iterator;
  while (it.moveNext()) {
    print('manual ${it.current}');
  }
  print(it.moveNext());
}
