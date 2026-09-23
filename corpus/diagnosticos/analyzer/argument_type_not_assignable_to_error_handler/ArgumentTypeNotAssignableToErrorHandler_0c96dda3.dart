void f(Stream<void> stream, void Function(dynamic a) callback) {
  stream.listen((_) {}, onError: callback);
}
