// Objects that escape and survive: binary trees and a linked list (real
// allocation and collection).
import 'common.dart';

class Node {
  final Node? left, right;
  Node(this.left, this.right);
  int count() => 1 + (left?.count() ?? 0) + (right?.count() ?? 0);
}

Node tree(int d) => d == 0 ? Node(null, null) : Node(tree(d - 1), tree(d - 1));

int trees(int maxD) {
  final longLived = tree(maxD);
  var total = 0;
  for (var d = 4; d <= maxD; d += 2) {
    final it = 1 << (maxD - d + 4);
    for (var i = 0; i < it; i++) {
      total += tree(d).count();
    }
  }
  return total + longLived.count();
}

class Link {
  final int v;
  Link? next;
  Link(this.v, this.next);
}

int list(int n) {
  Link? head;
  for (var i = 0; i < n; i++) {
    head = Link(i, head);
  }
  var s = 0;
  for (var e = head; e != null; e = e.next) {
    s += e.v;
  }
  return s;
}

void main() {
  measure('trees', () => trees(14));
  measure('linked_list', () => list(1000000));
}
