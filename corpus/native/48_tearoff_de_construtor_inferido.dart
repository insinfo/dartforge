// Tear-off de construtor de classe genérica com os argumentos de tipo
// inferidos pelo contexto (`putIfAbsent(k, HashSet.new)` do
// `build_resolvers`): o objeto nasce com os argumentos, como `C<T>.new`.
import 'dart:collection';

class Caixa<T> {
  final List<T> itens = [];
  Caixa();
  Caixa.com(T x) { itens.add(x); }
}

void main() {
  final m = <String, HashSet<int>>{};
  final s = m.putIfAbsent('a', HashSet.new);
  print(s.runtimeType);
  final c = <String, Caixa<double>>{};
  print(c.putIfAbsent('b', Caixa.new).runtimeType);
  Caixa<String> Function(String) f = Caixa.com;
  print(f('z').runtimeType);
  final g = Caixa<int>.new;
  print(g().runtimeType);
  final l = <String, List<int>>{};
  print(l.putIfAbsent('c', List.empty).runtimeType);
}
