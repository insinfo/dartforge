import 'dart:convert';

class Payload {
  final int value;
  Payload(this.value);
  @override
  String toString() => 'payload:$value';
}

int effects = 0;

Object? counted(Object? value) {
  effects++;
  return value;
}

void main() {
  final values = <Object?>[
    null,
    -4611686018427387904,
    -1,
    0,
    1,
    4611686018427387903,
    -4611686018427387905,
    4611686018427387904,
    true,
    false,
    1.5,
    'heap value',
    Payload(9),
  ];
  final copies = <Object?>[];
  for (final value in values) {
    copies.add(counted(value));
  }
  final callbacks = [for (final value in copies) () => value];
  values.clear();
  for (final callback in callbacks) {
    print(callback());
  }
  print('effects=$effects');
  print(identical(callbacks[8](), true));
  print(identical(callbacks[9](), false));
  print(jsonEncode({'small': counted(17), 'boolean': counted(false), 'null': counted(null)}));
  print('effects=$effects');
  copies.clear();
  print(callbacks[3]());
}
