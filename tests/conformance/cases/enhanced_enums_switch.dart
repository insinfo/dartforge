// Fixture original, com oracle Dart VM 3.6.2.
abstract class Described { String describe(); }
enum Mode implements Described {
  slow('slow', 1), fast('fast', 3);
  final String text;
  final int code;
  const Mode(this.text, this.code);
  String get label => this.text + '!';
  String describe() { return this.label; }
}
class PlainDescription implements Described {
  String describe() { return 'class'; }
}
String classify(Mode mode) => switch (mode) {
  Mode.slow => 'small',
  Mode selected when selected.code > 2 => selected.label,
  _ => 'fallback',
};
int observe(int value) { print(value); return value; }
void main() {
  Described described = Mode.fast;
  print(described.describe());
  print(switch (Mode.fast) {
    Described value when value.describe() == 'fast!' => 'interface',
    _ => 'missing',
  });
  print(switch (PlainDescription()) {
    Described value => value.describe(),
    _ => 'missing',
  });
  print(Mode.fast.name);
  print(Mode.fast.index);
  print(classify(Mode.slow));
  print(classify(Mode.fast));
  print(switch (observe(2)) { 1 => 'one', int n when n > 1 => 'many', _ => 'none' });
  switch (Mode.fast) {
    case Mode.fast when false: print('unreachable');
    case Mode.fast: print('fast case');
    case Mode.slow: print('slow case');
  }
  switch (observe(7)) {
    case int value when value > 5: print(value + 1);
    case _: print('fallback case');
  }
  for (var i = 0; i < 3; i++) {
    switch (i) {
      case 0: continue;
      case 1: print(31); break;
      case int value when value > 1: print(30 + value);
      case _: print(0);
    }
    print(100);
  }
}
