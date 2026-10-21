// Parameters that every call leaves `null` (omitted, or the `null` literal,
// or another parameter that is always `null`): the production profile proves
// it, as dart2js's inference does, and drops the branches that test them.
// Each proof has a counterexample next to it where one call passes a value,
// a tear-off escapes, a dynamic call reaches the method, or the body writes
// the parameter: those branches must stay and run.

class Selector {
  final String text;
  Selector.parse(this.text);
  @override
  String toString() => 'Selector($text)';
}

class Metadata {
  final String platform;
  final String timeout;

  // `testOn` is never passed: `Selector.parse` is dead.
  Metadata.parse({String? testOn, String? timeout})
      : platform = testOn == null ? 'all' : Selector.parse(testOn).toString(),
        timeout = timeout ?? 'default';

  // Redirects with an explicit `null`.
  Metadata.plain() : this.parse(testOn: null);
}

class Declarer {
  final List<String> log = [];

  // Instance method: every call site of the selector `test` leaves `testOn`
  // and `timeout` null, except through `timeout` in one site.
  void test(String name, {String? testOn, String? timeout}) {
    var m = Metadata.parse(testOn: testOn, timeout: timeout);
    if (testOn != null) {
      log.add('only on $testOn');
    }
    log.add('$name [${m.platform}, ${m.timeout}]');
  }
}

class VerboseDeclarer extends Declarer {
  @override
  void test(String name, {String? testOn, String? timeout}) {
    if (testOn != null) log.add('verbose on $testOn');
    super.test('verbose $name', testOn: testOn, timeout: timeout);
  }
}

final Declarer _declarer = Declarer();
final Declarer _verbose = VerboseDeclarer();

void test(String name, {String? testOn, String? timeout}) {
  _declarer.test(name, testOn: testOn, timeout: timeout);
}

// Counterexample: one call passes a title.
String greet(String name, [String? title]) {
  if (title != null) return 'Hello, $title $name';
  return 'Hello, $name';
}

// Counterexample: the body writes the parameter.
String written([int? x]) {
  x ??= 3;
  if (x != null) return 'written $x';
  return 'written null';
}

// Counterexample: a closure writes the parameter.
String capturedWrite([int? x]) {
  void set() {
    x = 7;
  }

  set();
  return x == null ? 'captured null' : 'captured $x';
}

// A closure only reads it: still always null.
String capturedRead([int? x]) {
  String read() => x == null ? 'read null' : 'read $x';
  return read();
}

// Counterexample: a non-null default.
String defaulted([int? x = 5]) => x != null ? 'default $x' : 'default null';

// Counterexample: the function escapes as a tear-off.
String escaped({String? tag}) => tag == null ? 'untagged' : 'tagged $tag';

abstract class Shape {
  String describe({String? suffix});
}

class Square implements Shape {
  @override
  String describe({String? suffix}) =>
      suffix != null ? 'square $suffix' : 'square';
}

class Circle implements Shape {
  @override
  String describe({String? suffix}) =>
      suffix != null ? 'circle $suffix' : 'circle';
}

// Counterexample: the selector `render` is reached by a dynamic call with a
// value.
class Widget {
  String render({String? prefix}) => prefix == null ? 'widget' : '$prefix widget';
}

class Panel {
  String render({String? prefix}) => prefix == null ? 'panel' : '$prefix panel';
}

// Counterexample: one virtual call passes a value to an override.
abstract class Node {
  String label([String? mark]);
}

class Leaf extends Node {
  @override
  String label([String? mark]) => mark == null ? 'leaf' : 'leaf$mark';
}

class Branch extends Node {
  @override
  String label([String? mark]) => mark == null ? 'branch' : 'branch$mark';
}

// Counterexample: `Function.apply` on a tear-off.
String applied({int? n}) => n == null ? 'applied null' : 'applied $n';

// A chain: the outer parameter is always null, so the inner one is too.
String outer({String? extra}) => inner(extra);
String inner(String? extra) => extra == null ? 'inner plain' : 'inner $extra';

// Counterexample chain: one call of the outer passes a value.
String outer2({String? extra}) => inner2(extra);
String inner2(String? extra) => extra == null ? 'inner2 plain' : 'inner2 $extra';

void main() {
  test('first');
  test('second', timeout: '2x');
  _verbose.test('third');
  _declarer.log.forEach(print);
  _verbose.log.forEach(print);
  var plain = Metadata.plain();
  print('${plain.platform} ${plain.timeout}');

  print(greet('Ada'));
  print(greet('Grace', 'Dr.'));
  print(written());
  print(capturedWrite());
  print(capturedRead());
  print(defaulted());

  var f = escaped;
  print(escaped());
  print(f(tag: 'x'));

  for (Shape s in [Square(), Circle()]) {
    print(s.describe());
  }

  dynamic w = Widget();
  print(w.render(prefix: 'dynamic'));
  print(Widget().render());
  print(Panel().render());

  Node n = Leaf();
  print(n.label());
  print(Branch().label('!'));

  print(Function.apply(applied, [], {#n: 4}));
  print(applied());

  print(outer());
  print(outer2());
  print(outer2(extra: 'more'));
}
