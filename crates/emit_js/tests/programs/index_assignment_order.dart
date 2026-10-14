import 'dart:typed_data';

final events = <String>[];
List<int> globalTarget = [0];

class Box {
  final String name;
  int value;
  void Function()? onRead;
  bool fail = false;
  Box(this.name, this.value);
  int operator [](int index) {
    events.add('$name.get:$index');
    onRead?.call();
    return value;
  }
  void operator []=(int index, int next) {
    events.add('$name.set:$index:$next');
    if (fail) throw StateError('setter');
    value = next;
  }
}

class NullableBox {
  final String name;
  int? value;
  void Function()? onRead;
  NullableBox(this.name, this.value);
  int? operator [](int index) {
    events.add('$name.get:$index');
    onRead?.call();
    return value;
  }
  void operator []=(int index, int? next) {
    events.add('$name.set:$index:$next');
    value = next;
  }
}

class Token {
  int value;
  Token(this.value);
}
extension TokenIndex on Token {
  int operator [](int index) => value;
  void operator []=(int index, int next) { value = next; }
}

extension type Wrapped(List<int> values) {
  int operator [](int index) => values[index];
  void operator []=(int index, int next) { values[index] = next; }
}

void output(String name) {
  print('$name: ${events.join(',')}');
  events.clear();
}

void main() {
  {
    Uint8List target = Uint8List(1);
    final original = target;
    int rhs() { target = Uint8List(1); return 7; }
    final result = (target[0] = rhs());
    print('typed: $original $target $result');
  }
  {
    var target = [0];
    final original = target;
    int index() { target = [0]; return 0; }
    target[index()] = 3;
    print('index-receiver: $original $target');
  }
  {
    final target = [0, 0];
    var index = 0;
    int rhs() { index = 1; return 5; }
    target[index] = rhs();
    print('index-rhs: $target $index');
  }
  {
    final original = globalTarget;
    int rhs() { globalTarget = [0]; return 6; }
    globalTarget[0] = rhs();
    print('global: $original $globalTarget');
  }
  {
    final first = Box('first', 4), second = Box('second', 10);
    var target = first, index = 0;
    first.onRead = () { target = second; index = 1; };
    int rhs() { events.add('rhs'); return 2; }
    final result = (target[index] += rhs());
    print('compound: ${first.value} ${second.value} $result');
    output('compound-order');
  }
  {
    final target = Box('post', 4);
    var index = 0;
    target.onRead = () { index = 1; };
    final result = target[index]++;
    print('post: ${target.value} $result');
    output('post-order');
  }
  {
    final first = NullableBox('null', null), second = NullableBox('other', 8);
    var target = first, index = 0;
    first.onRead = () { target = second; index = 1; };
    int rhs() { events.add('rhs'); return 9; }
    final result = (target[index] ??= rhs());
    print('ifnull: ${first.value} ${second.value} $result');
    output('ifnull-order');
  }
  {
    final target = NullableBox('full', 4);
    int rhs() { events.add('unexpected-rhs'); return 9; }
    print('full: ${target[0] ??= rhs()}');
    output('full-order');
  }
  {
    dynamic target = Box('dynamic', 0);
    final result = (target[0] = 11);
    print('dynamic-result: $result');
    output('dynamic-order');
  }
  {
    final first = Box('throw', 0), second = Box('replacement', 0);
    var target = first;
    first.fail = true;
    int rhs() { events.add('rhs'); target = second; return 3; }
    try { target[0] = rhs(); } catch (error) { print('setter-error: ${error is StateError}'); }
    print('setter-values: ${first.value} ${second.value}');
    output('setter-error-order');
  }
  {
    final target = [0];
    int index() { events.add('index'); return 2; }
    int rhs() { events.add('rhs'); return 1; }
    try { target[index()] = rhs(); } catch (error) { print('bounds: ${error is RangeError}'); }
    output('bounds-order');
  }
  {
    dynamic target = null;
    int index() { events.add('index'); return 0; }
    int rhs() { events.add('rhs'); return 1; }
    try { target[index()] = rhs(); } catch (error) { print('null: ${error is NoSuchMethodError}'); }
    output('null-order');
  }
  {
    final target = Box('type', 0);
    int index() { events.add('index'); return 0; }
    dynamic rhs() { events.add('rhs'); return 'bad'; }
    try { target[index()] = rhs(); } catch (error) { print('type: ${error is TypeError}'); }
    output('type-order');
  }
  {
    final first = Token(0);
    var target = first;
    int rhs() { target = Token(0); return 12; }
    final result = (target[0] = rhs());
    print('extension: ${first.value} ${target.value} $result');
  }
  {
    var target = Wrapped([0]);
    final first = target;
    int rhs() { target = Wrapped([0]); return 13; }
    final result = (target[0] = rhs());
    print('extension-type: ${first[0]} ${target[0]} $result');
    target = first;
    final compound = (target[0] += rhs());
    print('extension-type-compound: ${first[0]} ${target[0]} $compound');
  }
  {
    var target = [0];
    final original = target;
    var index = 0;
    int rhs() { events.add('conditional-rhs'); target = [0]; index = 1; return 15; }
    final condition = target.length == 1;
    final result = ((target)[index] = condition ? rhs() : 99);
    print('parenthesized-conditional: $original $target $index $result');
    output('conditional-order');
  }
  {
    final target = Box('no-set', 0);
    Box receiver() { events.add('receiver'); return target; }
    int index() { events.add('index'); return 0; }
    int rhs() { events.add('rhs'); throw StateError('rhs'); }
    try { receiver()[index()] = rhs(); } catch (error) { print('rhs-error: ${error is StateError}'); }
    output('rhs-error-order');
  }
  {
    List<int>? target;
    int index() { events.add('unexpected-index'); return 0; }
    int rhs() { events.add('unexpected-rhs'); return 1; }
    final result = (target?[index()] = rhs());
    print('null-aware: $result');
    output('null-aware-order');
  }

}
