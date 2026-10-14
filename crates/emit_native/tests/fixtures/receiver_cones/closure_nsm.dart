class Receiver {
  int get length => 7;
  int readLength() => length;
  int Function() capture() => () => length;
}

class Fallback {
  @override
  dynamic noSuchMethod(Invocation invocation) =>
      invocation.memberName == #length ? 11 : -1;
}

void main() {
  final receiver = Receiver();
  final callback = receiver.capture();
  print(callback());
  final getter = () => receiver.length;
  print(getter());
  final tearOff = receiver.readLength;
  print(tearOff());
  final dynamic fallback = Fallback();
  print(fallback.length);
}
