// A `late final` variable without an initializer has an implicit setter that
// may run once: inside a closure of its class, through `dynamic`, from a static
// or a top-level `late final`. A second write throws `LateInitializationError`
// (the VM's text). `dart:io`'s `_RawSecureSocket._socketSubscription` and
// `build_resolvers`' `_resolver` are fields like these: without the setter the
// write fell into `noSuchMethod`.

class Box {
  late final int value;
  late final String _label;

  void Function() later(int v) => () {
        value = v;
        _label = 'box $v';
      };

  String get label => _label;
}

class Holder {
  static late final List<int> numbers;
}

late final String topLevel;

String attempt(void Function() write) {
  try {
    write();
    return 'written';
  } on Error catch (e) {
    return e.runtimeType.toString().contains('Late') ? 'LateError' : '$e';
  }
}

void main() {
  final b = Box();
  b.later(7)();
  print('${b.value} ${b.label}');
  print(attempt(b.later(8)));
  final dynamic d = Box();
  d.value = 3;
  print(d.value);
  print(attempt(() => d.value = 4));
  Holder.numbers = [1, 2];
  print(Holder.numbers);
  print(attempt(() => Holder.numbers = []));
  topLevel = 'top';
  print(topLevel);
  print(attempt(() => topLevel = 'again'));
}
