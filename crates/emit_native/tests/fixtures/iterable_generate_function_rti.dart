void main() {
  print(Iterable<int>.generate(3).toList());
  try {
    print(Iterable<String>.generate(1).toList());
  } catch (e) {
    print(e is ArgumentError);
  }
  print(Iterable<String>.generate(2, (i) => 'v$i').toList());
}
