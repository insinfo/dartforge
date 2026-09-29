// Objetos do usuário guardados em coleções: `List<Item>` que cresce, é
// percorrida e ordenada por um campo, e `Map<int, Item>` /
// `Map<String, Item>` com objetos como valores (o que um programa de
// negócio faz: carregar registros, indexar, agregar).
import 'comum.dart';

class Item {
  final int id;
  final String nome;
  double preco;
  int quantidade;
  Item(this.id, this.nome, this.preco, this.quantidade);
  double get total => preco * quantidade;
}

// Os nomes são feitos uma vez: a medida é a dos objetos e das coleções,
// não a da interpolação.
final nomes = List<String>.generate(1000, (i) => 'item$i');

List<Item> carregar(int n) {
  final l = <Item>[];
  for (var i = 0; i < n; i++) {
    l.add(Item(i, nomes[i % 1000], (i % 97) * 1.5, i % 13));
  }
  return l;
}

double lista(int n) {
  final l = carregar(n);
  var s = 0.0;
  for (final it in l) {
    s += it.total;
  }
  return s;
}

int ordenar(int n) {
  final l = carregar(n);
  l.sort((a, b) => a.quantidade != b.quantidade ? a.quantidade - b.quantidade : b.id - a.id);
  return l[n ~/ 2].id;
}

double mapaInt(int n) {
  final m = <int, Item>{};
  for (var i = 0; i < n; i++) {
    m[i * 7 % n] = Item(i, 'x', i * 0.5, 1);
  }
  var s = 0.0;
  for (var i = 0; i < n; i++) {
    final it = m[i];
    if (it != null) {
      it.quantidade += 1;
      s += it.total;
    }
  }
  return s;
}

int mapaStr(int n) {
  final m = <String, Item>{};
  final l = carregar(n);
  for (final it in l) {
    final atual = m[it.nome];
    if (atual == null) {
      m[it.nome] = Item(it.id, it.nome, it.preco, it.quantidade);
    } else {
      atual.quantidade += it.quantidade;
    }
  }
  var q = 0;
  for (final it in m.values) {
    q += it.quantidade;
  }
  return q;
}

void main() {
  medir('lista_objetos', () => lista(300000));
  medir('ordenar_objetos', () => ordenar(200000));
  medir('mapa_int_objeto', () => mapaInt(200000));
  medir('mapa_str_objeto', () => mapaStr(200000));
}
