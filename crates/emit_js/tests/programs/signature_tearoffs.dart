// Tear-offs through the extension symbol (`dart.bind(o, $name)`) of SDK
// classes that implement a native interface without being native: the bound
// function keeps its signature, so dynamic calls check their arguments.
import 'dart:collection';

class Bag<E> extends ListBase<E> {
  final List<E> _items = [];
  @override
  int get length => _items.length;
  @override
  set length(int n) => _items.length = n;
  @override
  E operator [](int i) => _items[i];
  @override
  void operator []=(int i, E v) => _items[i] = v;
  @override
  void add(E e) => _items.add(e);
}

void check(String label, Function f, List<Object?> args) {
  try {
    print('$label: ${Function.apply(f, args)}');
  } on TypeError {
    print('$label: TypeError');
  }
}

void main() {
  final lists = <List<int>>[
    UnmodifiableListView<int>([1, 2, 3]),
    [4, 5, 6],
    Bag<int>()..addAll([7, 8, 9]),
  ];
  for (final l in lists) {
    final at = l.elementAt;
    print(at(1));
    print(at.runtimeType);
    check('elementAt', at, [2]);
    check('elementAt', at, ['x']);
    final contains = l.contains;
    check('contains', contains, ['x']);
  }
  final Iterable<String> words = UnmodifiableListView<String>(['a', 'bb']);
  final join = words.join;
  check('join', join, ['-']);
  check('join', join, [3]);
}
