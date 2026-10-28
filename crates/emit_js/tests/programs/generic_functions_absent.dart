// Function types without type parameters only: the production profile drops
// the substitution of generic function types; every type it prints and every
// test stays the VM's.

class Pair<A, B> {
  final A first;
  final B second;
  Pair(this.first, this.second);
  B Function(A) get swap => (A a) => second;
}

int length(String s) => s.length;

void main() {
  final p = Pair<int, String>(1, 'one');
  final f = p.swap;
  print(f(3));
  print(f.runtimeType);
  print(length.runtimeType);
  final Object o = (List<int> xs, [int? start]) => xs.length;
  print(o.runtimeType);
  print(o is Function);
  final handlers = <Object>[length, f, () => 1];
  print(handlers.map((h) => h.runtimeType.toString()).toList());
  print([3, 1, 2].map((x) => x * 2).toList());
  try {
    (o as Map<String, int>).length;
  } catch (e) {
    print(e is TypeError);
  }
}
