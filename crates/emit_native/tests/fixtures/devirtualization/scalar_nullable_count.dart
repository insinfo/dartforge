class Node {
  final Node? left;
  final Node? right;
  Node(this.left, this.right);
  int count() => 1 + (left?.count() ?? 0) + (right?.count() ?? 0);
}

class NumberSource {
  final int value;
  final bool fails;
  NumberSource(this.value, [this.fails = false]);
  int read() {
    if (fails) throw StateError('read failed');
    return value;
  }
}

int readNullable(NumberSource? source) => source?.read() ?? -7;

void main() {
  print(Node(Node(null, null), Node(Node(null, null), null)).count());
  for (final value in [0, -1, 4611686018427387904, -9223372036854775808, 9223372036854775807]) {
    print(readNullable(NumberSource(value)));
  }
  print(readNullable(null));
  try {
    print(readNullable(NumberSource(12, true)));
  } catch (_) {
    print('caught');
  }
}
