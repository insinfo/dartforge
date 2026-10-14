import 'dart:collection';

List<int> literalFactory() => [7, 14, 21];

List<int> filledFactory(bool growable) =>
    List<int>.filled(3, 7, growable: growable);

Object parameterFactory(Object value) => value;

class CustomList extends ListBase<int> {
  final List<int> _values = [7, 14, 21];

  @override
  int get length => _values.length;

  @override
  set length(int value) => _values.length = value;

  @override
  int operator [](int index) => _values[index];

  @override
  void operator []=(int index, int value) => _values[index] = value;
}

void main(List<String> args) {
  final literal = literalFactory();
  final fixed = filledFactory(false);
  final growable = filledFactory(true);
  growable.add(28);
  growable.removeLast();
  print(literal.length);
  print(fixed.length);
  print(growable.length);
  final alias = growable;
  final capture = () => alias;
  print(capture().length);
  dynamic opaque = parameterFactory(CustomList());
  print(opaque.length);
  print(opaque.join(','));
  print(literal.map((value) => value + 1).join(','));
}
