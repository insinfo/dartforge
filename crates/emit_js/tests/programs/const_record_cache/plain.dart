class Plain {
  final int number;
  const Plain(this.number);
}

const plain = Plain(7);
int value() => plain.number;
Object list() => const [1, 2, 3];

// A dead constant record must not retain a cache in this live library.
Object discarded() => const (99, name: 'dead');
