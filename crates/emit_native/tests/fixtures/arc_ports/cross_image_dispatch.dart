import 'dart:convert';

class Callable {
  final String prefix;

  Callable(this.prefix);

  String call(int value) => '$prefix:$value';
}

class Receiver {
  final String prefix;

  Receiver(this.prefix);

  String format(int value, {String suffix = 'end'}) => '$prefix:$value:$suffix';
}

class Missing {
  @override
  dynamic noSuchMethod(Invocation invocation) =>
      'missing:${invocation.positionalArguments.join(',')}';
}

String Function(int) makeFormatter(String prefix) =>
    (int value) => '$prefix:$value';

int Function(int) makeAdder(int offset) => (int value) => value + offset;

String invokeTyped(String Function(int) callback, int value) => callback(value);

int invokeScalar(int Function(int) callback, int value) => callback(value);

void main() {
  final values = [2, 4, 6];
  final iterator = values.iterator;
  final bool Function() sdkStep = iterator.moveNext;
  print('sdk:${sdkStep()}:${iterator.current}');
  print('sdk:${Function.apply(sdkStep, [])}:${iterator.current}');

  final formatter = makeFormatter('app');
  print('typed:${invokeTyped(formatter, 7)}');
  print('scalar:${invokeScalar(makeAdder(10), 3)}');
  print('map:${values.map(formatter).join('|')}');

  final dynamic callable = Callable('call');
  print('callable:${callable(9)}');
  final receiver = Receiver('method');
  final tearOff = receiver.format;
  print('tearoff:${tearOff(5)}');
  print('apply:${Function.apply(tearOff, [8], {#suffix: 'named'})}');

  final dynamic missing = Missing();
  print('fallback:${missing.absent('first', 'second')}');

  print('json:${jsonEncode({'items': values.map(formatter).toList()})}');

  try {
    values.map((int value) {
      if (value == 4) throw 'callback:$value';
      return 'before:$value';
    }).join('|');
  } catch (error) {
    print('caught:$error');
  }
  try {
    Function.apply((int value) => throw 'apply:$value', [11]);
  } catch (error) {
    print('caught:$error');
  }
  print('after:${values.map(formatter).join('|')}');
}
