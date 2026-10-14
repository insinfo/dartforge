class Node {}

class Parent extends Node {
  final List<Node?> children;

  Parent(this.children);
}

class Leaf extends Node {
  final int value;

  Leaf(this.value);
}

class SpecialLeaf extends Leaf {
  SpecialLeaf(int value) : super(value);
}

int? readNode(bool follow, List<Node?> children) {
  Node? node = Parent(children);
  if (follow) node = children[0];
  if (node is Leaf) return node.value;
  return null;
}

int? readKnownChild(bool follow) {
  Node? node = Parent([]);
  if (follow) node = SpecialLeaf(11);
  if (node is Leaf) return node.value;
  return null;
}

void main() {
  print(readNode(false, [Leaf(7)]));
  print(readNode(true, [Leaf(7)]));
  print(readNode(true, [Parent([])]));
  print(readNode(true, [null]));
  print(readNode(true, [SpecialLeaf(9)]));
  print(readKnownChild(false));
  print(readKnownChild(true));
}
