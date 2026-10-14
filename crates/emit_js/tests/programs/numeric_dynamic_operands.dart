final events = <String>[];

class Operand {
  @override
  String toString() {
    events.add('custom-toString');
    return '2';
  }
}

num left() { events.add('left'); return 6; }
int integerLeft() { events.add('left'); return 6; }
dynamic right(Object? value) { events.add('right'); return value; }

Object calculate(String op, Object? value) {
  switch (op) {
    case '+': return left() + right(value);
    case '-': return left() - right(value);
    case '*': return left() * right(value);
    case '/': return left() / right(value);
    case '~/': return left() ~/ right(value);
    case '%': return left() % right(value);
    case '<': return left() < right(value);
    case '>': return left() > right(value);
    case '<=': return left() <= right(value);
    case '>=': return left() >= right(value);
    case '&': return integerLeft() & right(value);
    case '|': return integerLeft() | right(value);
    case '^': return integerLeft() ^ right(value);
    case '<<': return integerLeft() << right(value);
    case '>>': return integerLeft() >> right(value);
    case '>>>': return integerLeft() >>> right(value);
  }
  throw StateError('Unknown operation');
}

class Slot {
  num value = 6;
  num get amount { events.add('get'); return value; }
  set amount(num next) { events.add('set'); value = next; }
}

num throwingLeft() { events.add('left-throw'); throw StateError('left'); }
dynamic throwingRight() { events.add('right-throw'); throw StateError('right'); }

String describe(Object value) => value is num ? value.toDouble().toStringAsFixed(2) : '$value';

void main() {
  final values = <Object?>[2, 2.5, '2', true, BigInt.from(2), Operand(), null];
  final labels = ['int', 'double', 'string', 'bool', 'bigint', 'custom', 'null'];
  for (final op in ['+', '-', '*', '/', '~/', '%', '<', '>', '<=', '>=', '&', '|', '^', '<<', '>>', '>>>']) {
    for (var i = 0; i < values.length; i++) {
      events.clear();
      try {
        print('$op:${labels[i]}=${describe(calculate(op, values[i]))}:$events');
      } catch (error) {
        print('$op:${labels[i]}=error:${error is TypeError}:$events');
      }
    }
  }
  for (final value in <Object?>[2, '2', null, Operand()]) {
    events.clear();
    final slot = Slot();
    try {
      slot.amount += right(value);
      print('compound=${describe(slot.value)}:$events');
    } catch (error) {
      print('compound=error:${error is TypeError}:${describe(slot.value)}:$events');
    }
  }
  events.clear();
  try { throwingLeft() + right('bad'); } catch (error) {
    print('left-throw=${error is StateError}:$events');
  }
  events.clear();
  try { left() + throwingRight(); } catch (error) {
    print('right-throw=${error is StateError}:$events');
  }
}
