// User objects kept in collections: a `List<Item>` that grows, is walked and
// sorted by a field, and `Map<int, Item>` / `Map<String, Item>` with objects
// as values (what a business program does: load records, index, aggregate).
import 'common.dart';

class Item {
  final int id;
  final String name;
  double price;
  int quantity;
  Item(this.id, this.name, this.price, this.quantity);
  double get total => price * quantity;
}

// The names are made once: the measurement is that of the objects and the
// collections, not of the interpolation.
final names = List<String>.generate(1000, (i) => 'item$i');

List<Item> load(int n) {
  final l = <Item>[];
  for (var i = 0; i < n; i++) {
    l.add(Item(i, names[i % 1000], (i % 97) * 1.5, i % 13));
  }
  return l;
}

double list(int n) {
  final l = load(n);
  var s = 0.0;
  for (final it in l) {
    s += it.total;
  }
  return s;
}

int sortItems(int n) {
  final l = load(n);
  l.sort((a, b) => a.quantity != b.quantity ? a.quantity - b.quantity : b.id - a.id);
  return l[n ~/ 2].id;
}

double mapInt(int n) {
  final m = <int, Item>{};
  for (var i = 0; i < n; i++) {
    m[i * 7 % n] = Item(i, 'x', i * 0.5, 1);
  }
  var s = 0.0;
  for (var i = 0; i < n; i++) {
    final it = m[i];
    if (it != null) {
      it.quantity += 1;
      s += it.total;
    }
  }
  return s;
}

int mapStr(int n) {
  final m = <String, Item>{};
  final l = load(n);
  for (final it in l) {
    final current = m[it.name];
    if (current == null) {
      m[it.name] = Item(it.id, it.name, it.price, it.quantity);
    } else {
      current.quantity += it.quantity;
    }
  }
  var q = 0;
  for (final it in m.values) {
    q += it.quantity;
  }
  return q;
}

void main() {
  measure('object_list', () => list(300000));
  measure('object_sort', () => sortItems(200000));
  measure('map_int_object', () => mapInt(200000));
  measure('map_str_object', () => mapStr(200000));
}
