class Values extends Iterable<int> {
  @override
  Iterator<int> get iterator => Counter();
}

class Counter implements Iterator<int> {
  int _position = 0;

  @override
  int get current => _position * 7;

  @override
  bool moveNext() => ++_position <= 3;
}

void main() {
  // The source SDK traverses the custom iterator through its protocol.
  print(Values().toList().join(','));
}
