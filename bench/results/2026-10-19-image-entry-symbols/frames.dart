import 'dart:async';
Future<int> sumLater(int value) async {
  final values = <int>[value, value + 1];
  await Future<void>.value();
  return values.fold<int>(0, (sum, item) => sum + item);
}
Iterable<int> numbers(int value) sync* {
  yield value;
  yield value + 1;
}
Stream<int> results(int value) async* {
  yield await sumLater(value);
  yield numbers(value).fold<int>(0, (sum, item) => sum + item);
}
Future<void> main() async {
  print(numbers(3).join(','));
  print(await sumLater(10));
  await for (final value in results(4)) {
    print(value);
  }
}
