// Implicit casts run as each literal element is evaluated, before later effects.
final events = <String>[];
dynamic value(String label, dynamic result) {
  events.add(label);
  return result;
}

void check(String label, Object? Function() build) {
  events.clear();
  try {
    print('$label: ${build()}');
  } on TypeError {
    print('$label: TypeError');
  }
  print(events.join(','));
}

List<T> generic<T>(dynamic input) => <T>[input];
Map<String, T> genericMap<T>(dynamic input) => <String, T>{'key': input};

void main() {
  check('list bad', () => <int>[
    value('first', 1), value('bad', 'x'), value('unreached', 3),
  ]);
  check('map value bad', () => <String, int>{
    value('key1', 'a'): value('value1', 1),
    value('key2', 'b'): value('bad', 'x'),
    value('unreached', 'c'): 3,
  });
  check('map key bad', () => <String, int>{
    value('bad', 1): value('unreached', 2),
  });
  check('list if', () => <int>[if (true) value('bad', 'x')]);
  check('list for', () => <int>[for (var i = 0; i < 1; i++) value('bad', 'x')]);
  check('map if', () => <String, int>{if (true) 'a': value('bad', 'x')});
  check('set', () => <int>{value('bad', 'x')});
  check('nullable', () => <int?>[value('null', null), value('int', 2)]);
  check('contextual', () { List<int> result = [value('bad', 'x')]; return result; });
  check('generic list', () => generic<int>('x'));
  check('generic map', () => genericMap<int>('x'));
  check('generic list valid', () => generic<int>(1));
  check('generic map valid', () => genericMap<int>(1));
  check('valid map', () => <String, int>{value('a', 'a'): value('one', 1), 'a': 2});
  check('covariance', () { List<Object?> result = <int>[1]; result[0] = 'x'; return result; });
}
