// Comparison between languages (RELATORIO-COMPARATIVO-NATIVO.md): the same
// five kernels in each one; `scripts/compare-languages.py` compiles and measures.
import 'dart:convert';

class Node {
  final Node? left, right;
  Node(this.left, this.right);
  int count() => 1 + (left?.count() ?? 0) + (right?.count() ?? 0);
}
Node tree(int d) => d == 0 ? Node(null, null) : Node(tree(d - 1), tree(d - 1));
int trees(int maxD) {
  final longLived = tree(maxD); var total = 0;
  for (var d = 4; d <= maxD; d += 2) { final it = 1 << (maxD - d + 4); for (var i = 0; i < it; i++) total += tree(d).count(); }
  return total + longLived.count();
}
int fib(int n) => n < 2 ? n : fib(n - 1) + fib(n - 2);
int numeric(int n) { var s = 0; for (var i = 0; i < n; i++) { s = (s + i * i) % 1000003; } return s; }
int strings(int n) {
  final m = <String, int>{};
  for (var i = 0; i < n; i++) { final k = 'k${i % 5000}'; m[k] = (m[k] ?? 0) + i; }
  var s = 0; m.forEach((_, v) { s += v % 1000; }); return s;
}
int json(int n) {
  final data = [
    for (var i = 0; i < n; i++)
      {'id': i, 'name': 'item$i', 'tags': ['a${i % 7}', 'b${i % 13}'], 'active': i % 2 == 0, 'level': i % 100}
  ];
  final read = jsonDecode(jsonEncode(data)) as List;
  var s = 0;
  for (final o in read) {
    final m = o as Map<String, dynamic>;
    s += (m['id'] as int) + (m['name'] as String).length + (m['tags'] as List).length + ((m['active'] as bool) ? 1 : 0) + (m['level'] as int);
  }
  return s;
}
void measure(String name, int Function() f) {
  var r = 0; final t = <int>[];
  for (var i = 0; i < 4; i++) { final sw = Stopwatch()..start(); r = f(); t.add(sw.elapsedMilliseconds); }
  print('$name: ${t.join(' ')} ms | $r');
}
void main() {
  measure('fib', () => fib(32));
  measure('trees', () => trees(14));
  measure('numeric', () => numeric(30000000));
  measure('strings', () => strings(1000000));
  measure('json', () => json(50000));
}
