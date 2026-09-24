void main() {
  dynamic text = 'foo';
  print(text + 'bar');
  try {
    print(text + 3);
  } catch (e) {
    print(e is TypeError);
  }
  print(text);
}
