class Holder {
  final String stored;
  Holder(this.stored);
  String get value => stored;
  String get failure => throw StateError('getter failure');
}

dynamic create() => Holder('getter value');

void probeFactory() {
  print(create().value);
  try {
    print(create().failure);
  } on StateError catch (error) {
    print(error.message);
  }
}

void main() {
  dynamic receiver = Holder('getter value');
  print(receiver.value);
  try {
    print(receiver.failure);
  } on StateError catch (error) {
    print(error.message);
  }
  probeFactory();
}
