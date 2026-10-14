import 'dart:typed_data';
import 'dart:collection';

final events = <String>[];
final doubles = Float64List.fromList([1.0, -0.0, double.nan, double.infinity]);

Float64List receiver() {
  events.add('receiver');
  return doubles;
}

int index() {
  events.add('index');
  doubles[0] = 7.0;
  return 0;
}

void check(List<num> values) {
  print('length=${values.length},first=${values[0]},last=${values[values.length - 1]}');
  for (final invalid in [-1, values.length, 0x100000000]) {
    try {
      values[invalid];
      throw StateError('accepted invalid index');
    } on RangeError catch (error) {
      print('range=${error.invalidValue}');
    }
  }
}

mixin SlotRead<T> {
  T get slot;
  int get length => 1;
  R fold<R>(R initial, R Function(R, T) combine) {
    events.add('fold-body');
    return combine(initial, slot);
  }
  T operator [](int index) {
    RangeError.checkValidIndex(index, this, null, length);
    return slot;
  }
}

class SlotList extends ListBase<int> with SlotRead<int> {
  int get slot => 42;
  set length(int value) => throw UnsupportedError('fixed');
  void operator []=(int index, int value) => throw UnsupportedError('fixed');
}

bool typedUnary(Object? value) => value is double Function(int);
bool typedFold(Object? value) => value is int Function(int, int Function(int, int));

void main() {
  check(Int8List.fromList([-128, 127]));
  check(Uint8List.fromList([0, 255]));
  check(Uint8ClampedList.fromList([-1, 300]));
  check(Int16List.fromList([-32768, 32767]));
  check(Uint16List.fromList([0, 65535]));
  check(Int32List.fromList([-2147483648, 2147483647]));
  check(Uint32List.fromList([0, 4294967295]));
  check(Float32List.fromList([1.5, 2.5]));
  check(Float64List.fromList([1.5, 2.5]));
  final view = Float64List.view(doubles.buffer, 8, 3);
  print('view=${view.length},negativeZero=${view[0].isNegative},nan=${view[1].isNaN},infinite=${view[2].isInfinite}');
  final readOnly = doubles.asUnmodifiableView();
  print('ordered=${receiver()[index()] == 7.0},events=${events.join(',')},readOnly=${readOnly[0] == 7.0}');
  doubles[0] = 9.0;
  print('shared=${readOnly[0] == 9.0}');
  final readOnlyElement = readOnly.elementAt;
  print('readOnlyTearoff=${typedUnary(readOnlyElement)},value=${readOnlyElement(0) == 9.0}');
  try {
    readOnly[0] = 3.0;
    throw StateError('mutable read-only view');
  } on UnsupportedError {
    print('read-only');
  }
  dynamic dynamicValues = doubles;
  for (final invalid in <dynamic>[1.5, '1', null]) {
    try {
      dynamicValues[invalid];
      throw StateError('accepted invalid index type');
    } catch (error) {
      print('type=${error is TypeError}');
    }
  }
  final custom = SlotList();
  check(custom);
  Iterable<int> asIterable = custom;
  final fold = asIterable.fold;
  final specialized = fold<int>;
  print('fold=${specialized(1, (a, b) => a + b)},typed=${typedFold(specialized)}');
  dynamic dynamicFold = fold;
  print('dynamicFold=${dynamicFold<int>(1, (int a, int b) => a + b)},foldEvents=${events.where((e) => e == 'fold-body').length}');
  print('customRead=${custom.elementAt(0)}');
  final integers = Uint8List.fromList([1, 2]).asUnmodifiableView();
  check(integers);
}
