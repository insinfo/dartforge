import 'dart:convert';

class Node {
  final int value;
  List<Node> links = <Node>[];
  Node(this.value);
}

int round(int seed, List<Node> roots) {
  final nodes = List<Node>.generate(32, (index) => Node(seed + index));
  for (var index = 0; index < nodes.length; index++) {
    final next = nodes[(index + 1) % nodes.length];
    nodes[index].links = <Node>[next, next, nodes[index]];
  }
  final encoded = jsonEncode(<String, Object>{
    'seed': seed,
    'values': nodes.map((node) => node.value).toList(),
    'nested': <Object?>[<int>[seed, seed + 1], true, null, 'bytes'],
  });
  final decoded = jsonDecode(encoded) as Map<String, dynamic>;
  final values = decoded['values'] as List<dynamic>;
  var sum = 0;
  for (final value in values) {
    sum += value as int;
  }
  for (final node in nodes) {
    if (!identical(node.links[0], node.links[1]) ||
        !identical(node.links[2], node)) {
      throw StateError('Duplicate edge identity changed');
    }
    sum += node.links[0].value;
  }
  roots.add(nodes[seed % nodes.length]);
  roots.add(nodes[seed % nodes.length]);
  if (!identical(roots[roots.length - 1], roots[roots.length - 2])) {
    throw StateError('Root identity changed');
  }
  return sum + (decoded['seed'] as int);
}

void main() {
  final roots = <Node>[];
  var checksum = 0;
  var retained = 0;
  for (var batch = 0; batch < 16; batch++) {
    checksum += round(batch * 37, roots);
    for (final root in roots) {
      retained += root.value + root.links[0].value;
    }
    if (batch % 4 == 3) {
      roots.clear();
    }
  }
  print('checksum=$checksum retained=$retained roots=${roots.length}');
  print(jsonEncode(<Object>[checksum, retained, roots.length]));
}
