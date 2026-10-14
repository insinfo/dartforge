import 'dart:_foreign_helper' as foreign;

List<int> rawMutation() {
  final output = <int>[];
  foreign.JS('', 'globalThis.__dartforgeCounterMutation = false');
  for (var i = 0; i < 3; i++) {
    foreign.JS('', '# = (globalThis.__dartforgeCounterMutation ? 4 : (globalThis.__dartforgeCounterMutation = true, -1))', i);
    output.add(i % 3);
  }
  return output;
}
void main() { print(rawMutation()); }
