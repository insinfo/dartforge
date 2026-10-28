// Storage symbols and the runtime's own symbols carry no description in
// production; every message, field and type the program prints stays the
// VM's.

class Box {
  final int value;
  String label;
  int _secret = 7;
  Box(this.value, this.label);
}

class Child extends Box {
  Child() : super(1, 'child');
  @override
  int get value => 42;
}

void main() {
  final b = Box(3, 'box');
  print([b.value, b.label, Child().value, Child().label]);
  b.label = 'changed';
  print(b.label);
  dynamic d = b;
  print(d.value);
  try {
    d._missing();
  } on NoSuchMethodError catch (e) {
    print(e.toString().contains('_missing'));
  }
  try {
    d.missing;
  } on NoSuchMethodError catch (e) {
    print(e.toString().contains('missing'));
  }
  print(d._secret);
  final errors = <Object>[
    ArgumentError.value(5, 'name', 'message'),
    RangeError.range(9, 0, 3, 'index'),
    IndexError.withLength(4, 2),
    StateError('state'),
    UnsupportedError('unsupported'),
    FormatException('format', 'source', 2),
  ];
  for (final e in errors) {
    print(e);
  }
  print((errors[0] as ArgumentError).message);
  print((errors[1] as RangeError).end);
  print([b.runtimeType, Child().runtimeType, errors.runtimeType]);
}
