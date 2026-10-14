import 'dart:collection';

class Numbers extends ListBase<int> {
  @override
  int get length => 2;

  @override
  set length(int value) => throw UnsupportedError('fixed length');

  @override
  int operator [](int index) => (index + 1) * 7;

  @override
  void operator []=(int index, int value) =>
      throw UnsupportedError('read only');
}

mixin ExtraLength on ListBase<int> {
  @override
  int get length => 3;
}

class MixedNumbers extends Numbers with ExtraLength {}

void main() {
  final values = MixedNumbers();
  print(values.isEmpty);
  print(values.length);
  print(values.join(','));
  print(values.take(2).join(','));
}
