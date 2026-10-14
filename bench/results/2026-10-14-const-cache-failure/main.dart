class Leaf { final int value; const Leaf(this.value); }
class Parent { final Leaf child = const Leaf(7); const Parent(); }
void main() { print(const Parent().child.value); }
